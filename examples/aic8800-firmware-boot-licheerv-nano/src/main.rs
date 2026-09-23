#![no_std]
#![no_main]

extern crate axplat_riscv64_licheerv_nano;

mod network;

use axdriver_aic8800::association::{AicAssociationClient, AssociationEvent, ConnectParameters};
use axdriver_aic8800::credentials::{parse_wifi_credentials, wifi_credential_wire_length};
use axdriver_aic8800::d80::{
    D80_CHIP_VERSION_ADDRESS, D80ExtPatchImage, load_d80_bluetooth, load_d80_wifi_and_start,
};
use axdriver_aic8800::data::EAPOL_ETHERTYPE;
use axdriver_aic8800::debug::AicDebugClient;
use axdriver_aic8800::device::Aic8800Product;
use axdriver_aic8800::eapol::{
    build_wpa2_psk_ccmp_message_2, build_wpa2_psk_ccmp_message_4, parse_wpa2_psk_ccmp_key_data,
    parse_wpa2_psk_ccmp_message_1, parse_wpa2_psk_ccmp_message_3,
};
use axdriver_aic8800::key::AicKeyClient;
use axdriver_aic8800::management::AicManagementClient;
use axdriver_aic8800::me::AicMeClient;
use axdriver_aic8800::patch_table::parse_patch_table;
use axdriver_aic8800::response::D80_MAXIMUM_RECEIVE_TRANSFER_LENGTH;
use axdriver_aic8800::rf::AicRfClient;
use axdriver_aic8800::rsn::{
    build_wpa2_psk_ccmp_station_information_element, parse_rsn_information_element,
};
use axdriver_aic8800::runtime::AicRuntimeClient;
use axdriver_aic8800::scan::{
    AicScanClient, ScanEvent, find_information_element, parse_bss_description,
};
use axdriver_aic8800::sdio::initialize_sdio_functions;
use axdriver_aic8800::sg2002::Sg2002SdioIo;
use axdriver_aic8800::tx::{
    EapolTransmitParameters, build_d80_eapol_data_transfer, send_d80_data_transfer,
};
use axdriver_aic8800::wpa_crypto::{derive_wpa2_ccmp_ptk, derive_wpa2_psk, unwrap_wpa2_key_data};
use axdriver_sg2002_sdio::enumeration::enumerate;
use axdriver_sg2002_sdio::sg2002::{host, register_interrupt};
use axstd::println;
use axstd::vec;
use axstd::vec::Vec;
use network::AicEthernetDevice;
use smoltcp::iface::{Config as InterfaceConfig, Interface, SocketSet};
use smoltcp::socket::dhcpv4;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpCidr};

const IDENTIFICATION_CLOCK_HZ: u32 = 400_000;
const FUNCTION_ENABLE_TIMEOUT_MS: u32 = 20;
const COMMAND_RESPONSE_TIMEOUT_MS: u32 = 6000;
const TARGET_SSID: &[u8] = b"MEIZU 20 Pro";
const RSN_INFORMATION_ELEMENT_ID: u8 = 48;
const EXPECTED_CHIP_VERSION_WORD: u32 = 0xf3c7_8820;
const FDRV_CHIP_ID_ADDRESS: u32 = 0x4050_0000;
const FDRV_CHIP_SUB_ID_ADDRESS: u32 = 0x0000_0020;

const PATCH_TABLE_NAME: &str = "fw_patch_table_8800d80_u02.bin";
const PATCH_TABLE_LENGTH: usize = 1384;
const ADID_NAME: &str = "fw_adid_8800d80_u02.bin";
const ADID_LENGTH: usize = 1708;
const BT_PATCH_NAME: &str = "fw_patch_8800d80_u02.bin";
const BT_PATCH_LENGTH: usize = 32700;
const BT_EXT0_NAME: &str = "fw_patch_8800d80_u02_ext0.bin";
const BT_EXT0_LENGTH: usize = 16136;
const WIFI_NAME: &str = "fmacfwbt_8800d80_h_u02.bin";
const WIFI_LENGTH: usize = 329580;

fn receive_exact(bytes: &mut [u8]) {
    let mut received = 0;
    while received < bytes.len() {
        let count = axhal::console::read_bytes(&mut bytes[received..]);
        if count == 0 {
            core::hint::spin_loop();
        } else {
            received += count;
        }
    }
}

fn crc32_ieee(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn receive_firmware(name: &str, length: usize) -> Option<Vec<u8>> {
    println!("READY AIC_FIRMWARE {name} {length}");
    let mut bytes = vec![0_u8; length];
    receive_exact(&mut bytes);
    let mut crc_bytes = [0_u8; 4];
    receive_exact(&mut crc_bytes);
    let transmitted_crc = u32::from_le_bytes(crc_bytes);
    let actual_crc = crc32_ieee(&bytes);
    if actual_crc != transmitted_crc {
        println!(
            "AIC8800_FIRMWARE_BOOT_FAILED crc name={name} expected={transmitted_crc:08x} actual={actual_crc:08x}"
        );
        return None;
    }
    println!("AIC_FIRMWARE_RECEIVED {name} crc32={actual_crc:08x}");
    Some(bytes)
}

#[unsafe(no_mangle)]
fn main() {
    println!("SG2002 AIC8800D80 full firmware boot test");

    if !register_interrupt() {
        println!("AIC8800_FIRMWARE_BOOT_FAILED irq-register");
        return;
    }
    let controller = host();
    if let Err(error) = controller.initialize(IDENTIFICATION_CLOCK_HZ) {
        println!("AIC8800_FIRMWARE_BOOT_FAILED host-init {error:?}");
        return;
    }
    let card = match enumerate(&controller) {
        Ok(card) => card,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED card-enumeration {error:?}");
            return;
        }
    };
    let function1 = match card
        .functions
        .iter()
        .flatten()
        .find(|function| function.number == 1)
    {
        Some(function) => function,
        None => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED function-1-missing");
            return;
        }
    };
    let product = match Aic8800Product::from_sdio_id(function1.vendor, function1.device) {
        Some(Aic8800Product::Aic8800D80) => Aic8800Product::Aic8800D80,
        value => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED product {value:?}");
            return;
        }
    };
    let mut io = match Sg2002SdioIo::new(&controller, FUNCTION_ENABLE_TIMEOUT_MS) {
        Ok(io) => io,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED adapter {error:?}");
            return;
        }
    };
    if let Err(error) = initialize_sdio_functions(&mut io, product) {
        println!("AIC8800_FIRMWARE_BOOT_FAILED function-init {error:?}");
        return;
    }

    let patch_table_bytes = match receive_firmware(PATCH_TABLE_NAME, PATCH_TABLE_LENGTH) {
        Some(bytes) => bytes,
        None => return,
    };
    let adid = match receive_firmware(ADID_NAME, ADID_LENGTH) {
        Some(bytes) => bytes,
        None => return,
    };
    let bt_patch = match receive_firmware(BT_PATCH_NAME, BT_PATCH_LENGTH) {
        Some(bytes) => bytes,
        None => return,
    };
    let bt_ext0 = match receive_firmware(BT_EXT0_NAME, BT_EXT0_LENGTH) {
        Some(bytes) => bytes,
        None => return,
    };
    let wifi = match receive_firmware(WIFI_NAME, WIFI_LENGTH) {
        Some(bytes) => bytes,
        None => return,
    };

    let patch_table = match parse_patch_table(&patch_table_bytes) {
        Ok(table) => table,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED patch-table {error:?}");
            return;
        }
    };
    let mut parameter = [0_u8; 1032];
    let mut transmit = [0_u8; 2048];
    let mut receive = [0_u8; D80_MAXIMUM_RECEIVE_TRANSFER_LENGTH];
    let mut client = AicDebugClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    let version_word = match client.read_word(D80_CHIP_VERSION_ADDRESS) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED chip-version {error:?}");
            return;
        }
    };
    if version_word != EXPECTED_CHIP_VERSION_WORD {
        println!(
            "AIC8800_FIRMWARE_BOOT_FAILED version expected={EXPECTED_CHIP_VERSION_WORD:#010x} actual={version_word:#010x}"
        );
        return;
    }

    let extensions = [D80ExtPatchImage {
        id: 0,
        bytes: &bt_ext0,
    }];
    println!("loading Bluetooth ADID, patch, extension and 140 table writes");
    if let Err(error) = load_d80_bluetooth(&mut client, &patch_table, &adid, &bt_patch, &extensions)
    {
        println!("AIC8800_FIRMWARE_BOOT_FAILED bluetooth {error:?}");
        return;
    }
    println!("Bluetooth firmware and patch table loaded");

    println!("loading Wi-Fi firmware and applying 3 patch pairs");
    let boot_status = match load_d80_wifi_and_start(&mut client, &wifi) {
        Ok(status) => status,
        Err(error) => {
            println!("AIC8800_FIRMWARE_BOOT_FAILED wifi {error:?}");
            return;
        }
    };
    println!("chip version = {version_word:#010x}");
    println!("Wi-Fi boot status = {boot_status:#010x}");
    println!("AIC8800_FIRMWARE_BOOT_PASS");

    let fdrv_chip_word = match client.read_word(FDRV_CHIP_ID_ADDRESS) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_STACK_FAILED chip-id {error:?}");
            return;
        }
    };
    let fdrv_chip_sub_word = match client.read_word(FDRV_CHIP_SUB_ID_ADDRESS) {
        Ok(value) => value,
        Err(error) => {
            println!("AIC8800_STACK_FAILED chip-sub-id {error:?}");
            return;
        }
    };
    println!("FDRV chip ID word     = {fdrv_chip_word:#010x}");
    println!(
        "FDRV chip ID          = {:#04x}",
        (fdrv_chip_word >> 16) as u8
    );
    println!("FDRV chip sub-ID word = {fdrv_chip_sub_word:#010x}");
    println!("FDRV chip sub-ID      = {:#04x}", fdrv_chip_sub_word as u8);

    drop(client);
    let mut management = AicManagementClient::new(
        &mut io,
        product,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    let stack = match management.start_d80_stack() {
        Ok(confirmation) => confirmation,
        Err(error) => {
            println!("AIC8800_STACK_FAILED start {error:?}");
            return;
        }
    };
    let firmware_build = match management.read_firmware_build_version() {
        Ok(version) => version,
        Err(error) => {
            println!("AIC8800_STACK_FAILED firmware-version {error:?}");
            return;
        }
    };
    println!("5 GHz supported       = {}", stack.supports_5ghz);
    println!("vendor info           = {:#04x}", stack.vendor_info);
    match core::str::from_utf8(firmware_build.as_bytes()) {
        Ok(text) => println!("firmware build        = {text}"),
        Err(_) => println!("firmware build        = <non-UTF-8>"),
    }
    println!("AIC8800_STACK_PASS");

    drop(management);
    let mut rf = AicRfClient::new(
        &mut io,
        product,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    let calibration = match rf.configure_d80_board() {
        Ok(addresses) => addresses,
        Err(error) => {
            println!("AIC8800_RF_FAILED {error:?}");
            return;
        }
    };
    println!("RF RX gain 2.4 GHz = {:#010x}", calibration.rx_gain_24g);
    println!("RF RX gain 5 GHz   = {:#010x}", calibration.rx_gain_5g);
    println!("RF TX gain 2.4 GHz = {:#010x}", calibration.tx_gain_24g);
    println!("RF TX gain 5 GHz   = {:#010x}", calibration.tx_gain_5g);

    drop(rf);
    let mut management = AicManagementClient::new(
        &mut io,
        product,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    let mac_address = match management.read_mac_address() {
        Ok(address) => address,
        Err(error) => {
            println!("AIC8800_MANAGEMENT_FAILED mac-address {error:?}");
            return;
        }
    };
    println!(
        "Wi-Fi MAC address  = {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5]
    );
    println!("AIC8800_RF_MAC_PASS");
    if let Err(error) = management.reset() {
        println!("AIC8800_MANAGEMENT_FAILED reset {error:?}");
        return;
    }
    let firmware_version = match management.read_version() {
        Ok(version) => version,
        Err(error) => {
            println!("AIC8800_MANAGEMENT_FAILED version {error:?}");
            return;
        }
    };
    println!(
        "LMAC version       = {:#010x}",
        firmware_version.version_lmac
    );
    println!(
        "MAC HW version 1   = {:#010x}",
        firmware_version.version_machw_1
    );
    println!(
        "MAC HW version 2   = {:#010x}",
        firmware_version.version_machw_2
    );
    println!(
        "PHY version 1      = {:#010x}",
        firmware_version.version_phy_1
    );
    println!(
        "PHY version 2      = {:#010x}",
        firmware_version.version_phy_2
    );
    println!("firmware features  = {:#010x}", firmware_version.features);
    println!("maximum stations   = {}", firmware_version.max_sta_nb);
    println!("maximum interfaces = {}", firmware_version.max_vif_nb);
    println!("AIC8800_MANAGEMENT_PASS");

    drop(management);
    let mut me = AicMeClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    if let Err(error) = me.configure_d80_board() {
        println!("AIC8800_ME_FAILED {error:?}");
        return;
    }
    println!("ME capabilities configured");
    println!("ME channels configured: 2.4 GHz=14, 5 GHz=25");
    println!("AIC8800_ME_PASS");

    drop(me);
    let mut runtime = AicRuntimeClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    if let Err(error) = runtime.start() {
        println!("AIC8800_STA_INTERFACE_FAILED start {error:?}");
        return;
    }
    let interface = match runtime.add_station_interface(mac_address) {
        Ok(confirmation) => confirmation,
        Err(error) => {
            println!("AIC8800_STA_INTERFACE_FAILED add {error:?}");
            return;
        }
    };
    if interface.status != 0 {
        println!("AIC8800_STA_INTERFACE_FAILED status={}", interface.status);
        return;
    }
    println!("STA interface index = {}", interface.interface_index);
    println!("AIC8800_STA_INTERFACE_PASS");

    drop(runtime);
    let mut scan = AicScanClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    if let Err(error) = scan.start_d80_full_scan(interface.interface_index) {
        println!("AIC8800_SCAN_FAILED start {error:?}");
        return;
    }
    println!("AIC8800_SCAN_STARTED");

    let mut observed_results = 0_u32;
    let mut observed_surveys = 0_u32;
    let mut target_bssid = None;
    let mut target_frequency_mhz = 0_u16;
    let mut target_band = 0_u8;
    let mut target_access_point_rsn = [0_u8; 257];
    let mut target_access_point_rsn_length = 0_usize;
    let mut target_station_rsn = [0_u8; 22];
    let mut target_station_rsn_length = 0_usize;
    loop {
        match scan.next_event() {
            Ok(ScanEvent::Result(result)) => {
                observed_results += 1;
                match parse_bss_description(result.frame) {
                    Ok(bss) => match core::str::from_utf8(bss.ssid) {
                        Ok(ssid) => {
                            println!(
                                "scan result #{observed_results}: ssid={ssid:?} bssid={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} freq={}MHz rssi={}dBm",
                                bss.bssid[0],
                                bss.bssid[1],
                                bss.bssid[2],
                                bss.bssid[3],
                                bss.bssid[4],
                                bss.bssid[5],
                                result.center_frequency_mhz,
                                result.rssi_dbm
                            );
                            if bss.ssid == TARGET_SSID {
                                match find_information_element(
                                    bss.information_elements,
                                    RSN_INFORMATION_ELEMENT_ID,
                                ) {
                                    Ok(Some(rsn)) => {
                                        println!("AIC8800_TARGET_RSN_IE {:02x?}", rsn);
                                        match parse_rsn_information_element(rsn) {
                                            Ok(parsed) => {
                                                match build_wpa2_psk_ccmp_station_information_element(
                                                    &mut target_station_rsn,
                                                    &parsed,
                                                ) {
                                                    Ok(length) => {
                                                        if rsn.len()
                                                            > target_access_point_rsn.len()
                                                        {
                                                            println!(
                                                                "AIC8800_TARGET_RSN_SELECTION_FAILED length={} maximum={}",
                                                                rsn.len(),
                                                                target_access_point_rsn.len()
                                                            );
                                                            continue;
                                                        }
                                                        target_bssid = Some(bss.bssid);
                                                        target_frequency_mhz =
                                                            result.center_frequency_mhz;
                                                        target_band = result.band;
                                                        target_access_point_rsn[..rsn.len()]
                                                            .copy_from_slice(rsn);
                                                        target_access_point_rsn_length = rsn.len();
                                                        target_station_rsn_length = length;
                                                        println!(
                                                            "AIC8800_TARGET_RSN_SELECTED WPA2-PSK CCMP"
                                                        );
                                                    }
                                                    Err(error) => println!(
                                                        "AIC8800_TARGET_RSN_SELECTION_FAILED {error:?}"
                                                    ),
                                                }
                                            }
                                            Err(error) => {
                                                println!("AIC8800_TARGET_RSN_PARSE_FAILED {error:?}")
                                            }
                                        }
                                    }
                                    Ok(None) => println!("AIC8800_TARGET_RSN_IE_MISSING"),
                                    Err(error) => {
                                        println!("AIC8800_TARGET_RSN_IE_INVALID {error:?}")
                                    }
                                }
                            }
                        }
                        Err(_) => println!(
                            "scan result #{observed_results}: ssid=<non-UTF-8> freq={}MHz rssi={}dBm frame_length={}",
                            result.center_frequency_mhz, result.rssi_dbm, result.length
                        ),
                    },
                    Err(error) => println!(
                        "scan result #{observed_results}: bss-decode={error:?} freq={}MHz rssi={}dBm frame_control={:#06x} frame_length={}",
                        result.center_frequency_mhz,
                        result.rssi_dbm,
                        result.frame_control,
                        result.length
                    ),
                }
            }
            Ok(ScanEvent::ChannelSurvey(survey)) => {
                observed_surveys += 1;
                println!(
                    "channel survey #{observed_surveys}: freq={}MHz noise={}dBm time={}ms busy={}ms",
                    survey.frequency_mhz,
                    survey.noise_dbm,
                    survey.channel_time_ms,
                    survey.channel_busy_time_ms
                );
            }
            Ok(ScanEvent::Complete(done)) => {
                println!(
                    "scan complete: interface={} status={} firmware_results={} observed_results={observed_results} observed_surveys={observed_surveys}",
                    done.interface_index, done.status, done.result_count,
                );
                if done.interface_index != interface.interface_index || done.status != 0 {
                    println!(
                        "AIC8800_SCAN_FAILED complete interface={} status={}",
                        done.interface_index, done.status
                    );
                    return;
                }
                println!("AIC8800_SCAN_PASS");
                break;
            }
            Err(error) => {
                println!("AIC8800_SCAN_FAILED event {error:?}");
                return;
            }
        }
    }

    let target_bssid = match target_bssid {
        Some(value) if target_access_point_rsn_length != 0 && target_station_rsn_length != 0 => {
            value
        }
        _ => {
            println!("AIC8800_ASSOCIATION_FAILED target-not-ready");
            return;
        }
    };
    drop(scan);

    let mut credential_storage = [0_u8; 100];
    println!("READY WIFI_CREDENTIALS PASSLEN_U8 SNONCE_32 CRC32_LE");
    receive_exact(&mut credential_storage[..1]);
    let credential_length = match wifi_credential_wire_length(credential_storage[0]) {
        Ok(length) => length,
        Err(error) => {
            println!("AIC8800_WIFI_CREDENTIALS_FAILED length {error:?}");
            return;
        }
    };
    receive_exact(&mut credential_storage[1..credential_length]);
    println!("AIC8800_WIFI_CREDENTIAL_FRAME_RECEIVED");
    let (pairwise_master_key, station_nonce) = {
        let credentials = match parse_wifi_credentials(&credential_storage[..credential_length]) {
            Ok(credentials) => credentials,
            Err(error) => {
                println!("AIC8800_WIFI_CREDENTIALS_FAILED frame {error:?}");
                return;
            }
        };
        println!("AIC8800_WIFI_CREDENTIALS_VALIDATED");
        println!("AIC8800_WIFI_PSK_DERIVE_STARTED");
        let pairwise_master_key = match derive_wpa2_psk(credentials.passphrase, TARGET_SSID) {
            Ok(key) => key,
            Err(error) => {
                println!("AIC8800_WIFI_CREDENTIALS_FAILED derive {error:?}");
                return;
            }
        };
        println!("AIC8800_WIFI_PSK_DERIVE_DONE");
        (pairwise_master_key, credentials.station_nonce)
    };
    credential_storage[..credential_length].fill(0);
    println!("AIC8800_WIFI_CREDENTIALS_RECEIVED");

    let mut association = AicAssociationClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    if let Err(error) = association.start_wpa2_personal_connect(ConnectParameters {
        ssid: TARGET_SSID,
        bssid: target_bssid,
        frequency_mhz: target_frequency_mhz,
        band: target_band,
        channel_flags: 0,
        transmit_power_dbm: 0,
        association_information_elements: &target_station_rsn[..target_station_rsn_length],
        interface_index: interface.interface_index,
    }) {
        println!("AIC8800_ASSOCIATION_FAILED start {error:?}");
        return;
    }
    println!(
        "AIC8800_ASSOCIATION_STARTED bssid={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} freq={}MHz",
        target_bssid[0],
        target_bssid[1],
        target_bssid[2],
        target_bssid[3],
        target_bssid[4],
        target_bssid[5],
        target_frequency_mhz
    );

    let mut confirmation_passed = false;
    let mut association_indices = None;
    let mut message_1_storage = [0_u8; 1536];
    let mut message_1_length = 0_usize;
    loop {
        match association.next_event() {
            Ok(AssociationEvent::Confirmation(confirmation)) => {
                println!(
                    "AIC8800_ASSOCIATION_CONFIRMATION status={}",
                    confirmation.status
                );
                if confirmation.status != 0 {
                    println!(
                        "AIC8800_ASSOCIATION_FAILED confirmation-status={}",
                        confirmation.status
                    );
                    return;
                }
                confirmation_passed = true;
            }
            Ok(AssociationEvent::Indication(indication)) => {
                println!(
                    "AIC8800_ASSOCIATION_INDICATION status={} aid={} ap-index={} channel-index={} qos={}",
                    indication.status_code,
                    indication.association_id,
                    indication.access_point_index,
                    indication.channel_index,
                    indication.qos
                );
                if indication.status_code != 0 {
                    println!(
                        "AIC8800_ASSOCIATION_FAILED indication-status={}",
                        indication.status_code
                    );
                    return;
                }
                association_indices =
                    Some((indication.interface_index, indication.access_point_index));
            }
            Ok(AssociationEvent::Data(frame)) => {
                println!(
                    "AIC8800_DATA_FRAME ethertype=0x{:04x} length={} qos={}",
                    frame.ether_type,
                    frame.payload.len(),
                    frame.qos
                );
                if frame.ether_type == EAPOL_ETHERTYPE {
                    match parse_wpa2_psk_ccmp_message_1(frame.payload) {
                        Ok(message) => {
                            println!(
                                "AIC8800_EAPOL_MESSAGE_1 version={} descriptor-version={} key-length={} replay-counter={} key-data-length={} trailing-length={}",
                                message.protocol_version,
                                message.descriptor_version,
                                message.key_length,
                                message.replay_counter,
                                message.key_data.len(),
                                message.trailing.len()
                            );
                            println!(
                                "AIC8800_EAPOL_MESSAGE_1_NONCE {:02x?}",
                                message.authenticator_nonce
                            );
                            if frame.payload.len() > message_1_storage.len() {
                                println!(
                                    "AIC8800_EAPOL_RX_FAILED message-1-too-long length={} maximum={}",
                                    frame.payload.len(),
                                    message_1_storage.len()
                                );
                                return;
                            }
                            message_1_storage[..frame.payload.len()].copy_from_slice(frame.payload);
                            message_1_length = frame.payload.len();
                        }
                        Err(error) => {
                            println!("AIC8800_EAPOL_RX_FAILED decode {error:?}");
                            return;
                        }
                    }
                }
            }
            Ok(AssociationEvent::UndecodedData { packet, error }) => {
                let prefix_length = core::cmp::min(packet.len(), 96);
                println!(
                    "AIC8800_UNDECODED_DATA error={error:?} packet-length={} prefix={:02x?}",
                    packet.len(),
                    &packet[..prefix_length]
                );
            }
            Ok(AssociationEvent::Transport { message_type }) => {
                println!("AIC8800_ASSOCIATION_TRANSPORT_MESSAGE type={message_type}");
            }
            Ok(AssociationEvent::Unrelated { message_id }) => {
                println!("AIC8800_ASSOCIATION_ASYNC_MESSAGE id={message_id}");
            }
            Err(error) => {
                println!("AIC8800_ASSOCIATION_FAILED event {error:?}");
                return;
            }
        }
        if confirmation_passed && association_indices.is_some() && message_1_length != 0 {
            break;
        }
    }
    if !confirmation_passed || association_indices.is_none() {
        println!(
            "AIC8800_ASSOCIATION_FAILED missing-event confirmation={confirmation_passed} indication={} ",
            association_indices.is_some()
        );
        return;
    }
    println!("AIC8800_ASSOCIATION_PASS");
    println!("AIC8800_EAPOL_RX_PASS");
    println!("AIC8800_EAPOL_MESSAGE_1_PASS");

    drop(association);
    let message_1 = match parse_wpa2_psk_ccmp_message_1(&message_1_storage[..message_1_length]) {
        Ok(message) => message,
        Err(error) => {
            println!("AIC8800_EAPOL_MESSAGE_2_FAILED message-1 {error:?}");
            return;
        }
    };
    let pairwise_transient_key = derive_wpa2_ccmp_ptk(
        &pairwise_master_key,
        target_bssid,
        mac_address,
        message_1.authenticator_nonce,
        station_nonce,
    );
    let mut message_2_storage = [0_u8; 512];
    let message_2_length = match build_wpa2_psk_ccmp_message_2(
        &mut message_2_storage,
        &message_1,
        station_nonce,
        &target_station_rsn[..target_station_rsn_length],
        &pairwise_transient_key,
    ) {
        Ok(length) => length,
        Err(error) => {
            println!("AIC8800_EAPOL_MESSAGE_2_FAILED build {error:?}");
            return;
        }
    };
    let (interface_index, station_index) = association_indices.unwrap();
    let mut data_transfer = [0_u8; 512];
    let data_lengths = match build_d80_eapol_data_transfer(
        &mut data_transfer,
        EapolTransmitParameters {
            destination_address: target_bssid,
            source_address: mac_address,
            interface_index,
            station_index,
            confirmation_index: 0,
        },
        &message_2_storage[..message_2_length],
    ) {
        Ok(lengths) => lengths,
        Err(error) => {
            println!("AIC8800_EAPOL_MESSAGE_2_FAILED transfer-build {error:?}");
            return;
        }
    };
    if let Err(error) =
        send_d80_data_transfer(&mut io, &mut data_transfer[..data_lengths.transfer_length])
    {
        println!("AIC8800_EAPOL_MESSAGE_2_FAILED send {error:?}");
        return;
    }
    message_2_storage[..message_2_length].fill(0);
    data_transfer.fill(0);
    println!(
        "AIC8800_EAPOL_MESSAGE_2_SENT eapol-length={} sdio-frame-length={} transfer-length={} vif-index={} station-index={}",
        message_2_length,
        data_lengths.frame_length,
        data_lengths.transfer_length,
        interface_index,
        station_index
    );

    let mut association = AicAssociationClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    let mut message_4_storage = [0_u8; 512];
    let mut decrypted_key_data = [0_u8; 512];
    let group_key_data = loop {
        match association.next_event() {
            Ok(AssociationEvent::Data(frame)) if frame.ether_type == EAPOL_ETHERTYPE => {
                match parse_wpa2_psk_ccmp_message_3(
                    frame.payload,
                    &message_1,
                    &pairwise_transient_key,
                ) {
                    Ok(message_3) => {
                        println!(
                            "AIC8800_EAPOL_MESSAGE_3 key-info={:#06x} replay-counter={} key-data-length={} encrypted={} install={} secure={}",
                            message_3.key_information,
                            message_3.replay_counter,
                            message_3.key_data.len(),
                            message_3.encrypted_key_data,
                            message_3.install,
                            message_3.secure
                        );
                        if !message_3.encrypted_key_data || !message_3.install || !message_3.secure
                        {
                            println!(
                                "AIC8800_EAPOL_MESSAGE_3_FAILED required-flags encrypted={} install={} secure={}",
                                message_3.encrypted_key_data, message_3.install, message_3.secure
                            );
                            return;
                        }
                        let decrypted_length = match unwrap_wpa2_key_data(
                            &mut decrypted_key_data,
                            message_3.key_data,
                            pairwise_transient_key.key_encryption_key(),
                        ) {
                            Ok(length) => length,
                            Err(error) => {
                                println!("AIC8800_EAPOL_MESSAGE_3_FAILED unwrap {error:?}");
                                return;
                            }
                        };
                        let group_key_data = match parse_wpa2_psk_ccmp_key_data(
                            &decrypted_key_data[..decrypted_length],
                            &target_access_point_rsn[..target_access_point_rsn_length],
                        ) {
                            Ok(key_data) => key_data,
                            Err(error) => {
                                decrypted_key_data[..decrypted_length].fill(0);
                                println!("AIC8800_EAPOL_MESSAGE_3_FAILED key-data {error:?}");
                                return;
                            }
                        };
                        let message_4_length = match build_wpa2_psk_ccmp_message_4(
                            &mut message_4_storage,
                            &message_3,
                            &pairwise_transient_key,
                        ) {
                            Ok(length) => length,
                            Err(error) => {
                                decrypted_key_data[..decrypted_length].fill(0);
                                println!("AIC8800_EAPOL_MESSAGE_4_FAILED build {error:?}");
                                return;
                            }
                        };
                        decrypted_key_data[..decrypted_length].fill(0);
                        println!("AIC8800_EAPOL_MESSAGE_2_PASS");
                        println!("AIC8800_EAPOL_MESSAGE_3_PASS");
                        break (message_4_length, group_key_data);
                    }
                    Err(error) => {
                        println!("AIC8800_EAPOL_MESSAGE_3_FAILED decode {error:?}");
                        return;
                    }
                }
            }
            Ok(AssociationEvent::Data(frame)) => println!(
                "AIC8800_POST_MESSAGE_2_DATA ethertype=0x{:04x} length={}",
                frame.ether_type,
                frame.payload.len()
            ),
            Ok(AssociationEvent::UndecodedData { packet, error }) => println!(
                "AIC8800_POST_MESSAGE_2_UNDECODED error={error:?} packet-length={}",
                packet.len()
            ),
            Ok(AssociationEvent::Transport { message_type }) => {
                println!("AIC8800_POST_MESSAGE_2_TRANSPORT type={message_type}")
            }
            Ok(AssociationEvent::Unrelated { message_id }) => {
                println!("AIC8800_POST_MESSAGE_2_ASYNC id={message_id}")
            }
            Ok(AssociationEvent::Confirmation(confirmation)) => println!(
                "AIC8800_POST_MESSAGE_2_CONFIRMATION status={}",
                confirmation.status
            ),
            Ok(AssociationEvent::Indication(indication)) => println!(
                "AIC8800_POST_MESSAGE_2_INDICATION status={}",
                indication.status_code
            ),
            Err(error) => {
                println!("AIC8800_EAPOL_MESSAGE_3_FAILED event {error:?}");
                return;
            }
        }
    };
    drop(association);

    let (message_4_length, group_key_data) = group_key_data;
    let data_lengths = match build_d80_eapol_data_transfer(
        &mut data_transfer,
        EapolTransmitParameters {
            destination_address: target_bssid,
            source_address: mac_address,
            interface_index,
            station_index,
            confirmation_index: 1,
        },
        &message_4_storage[..message_4_length],
    ) {
        Ok(lengths) => lengths,
        Err(error) => {
            println!("AIC8800_EAPOL_MESSAGE_4_FAILED transfer-build {error:?}");
            return;
        }
    };
    if let Err(error) =
        send_d80_data_transfer(&mut io, &mut data_transfer[..data_lengths.transfer_length])
    {
        println!("AIC8800_EAPOL_MESSAGE_4_FAILED send {error:?}");
        return;
    }
    message_4_storage[..message_4_length].fill(0);
    data_transfer.fill(0);
    println!(
        "AIC8800_EAPOL_MESSAGE_4_SENT eapol-length={} sdio-frame-length={} transfer-length={}",
        message_4_length, data_lengths.frame_length, data_lengths.transfer_length
    );
    println!("AIC8800_EAPOL_MESSAGE_4_PASS");

    let mut keys = AicKeyClient::new(
        &mut io,
        product,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    let pairwise_confirmation = match keys.install_pairwise_ccmp(
        interface_index,
        station_index,
        pairwise_transient_key.temporal_key(),
    ) {
        Ok(confirmation) => confirmation,
        Err(error) => {
            println!("AIC8800_PAIRWISE_KEY_FAILED command {error:?}");
            return;
        }
    };
    if pairwise_confirmation.status != 0 {
        println!(
            "AIC8800_PAIRWISE_KEY_FAILED status={}",
            pairwise_confirmation.status
        );
        return;
    }
    println!(
        "AIC8800_PAIRWISE_KEY_INSTALLED hardware-index={}",
        pairwise_confirmation.hardware_key_index
    );

    let group_confirmation = match keys.install_group_ccmp(
        interface_index,
        group_key_data.group_key_index,
        &group_key_data.group_temporal_key,
    ) {
        Ok(confirmation) => confirmation,
        Err(error) => {
            println!("AIC8800_GROUP_KEY_FAILED command {error:?}");
            return;
        }
    };
    if group_confirmation.status != 0 {
        println!(
            "AIC8800_GROUP_KEY_FAILED status={}",
            group_confirmation.status
        );
        return;
    }
    println!(
        "AIC8800_GROUP_KEY_INSTALLED key-index={} hardware-index={}",
        group_key_data.group_key_index, group_confirmation.hardware_key_index
    );
    println!("AIC8800_KEY_INSTALL_PASS");
    println!("AIC8800_LINK_UP_PASS");
    drop(keys);

    let mut me = AicMeClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        COMMAND_RESPONSE_TIMEOUT_MS,
    );
    if let Err(error) = me.set_control_port(station_index, true) {
        println!("AIC8800_CONTROL_PORT_OPEN_FAILED {error:?}");
        return;
    }
    println!("AIC8800_CONTROL_PORT_OPEN_PASS station-index={station_index}");
    drop(me);

    let network_client = AicAssociationClient::new(
        &mut io,
        product,
        &mut parameter,
        &mut transmit,
        &mut receive,
        0,
    );
    let mut network_device = AicEthernetDevice::new(network_client, interface_index, station_index);
    let mut interface_config =
        InterfaceConfig::new(HardwareAddress::Ethernet(EthernetAddress(mac_address)));
    interface_config.random_seed = u64::from_le_bytes([
        mac_address[0],
        mac_address[1],
        mac_address[2],
        mac_address[3],
        mac_address[4],
        mac_address[5],
        interface_index,
        station_index,
    ]);
    let start_nanoseconds = axhal::time::wall_time_nanos();
    let mut network_interface = Interface::new(
        interface_config,
        &mut network_device,
        Instant::from_micros_const((start_nanoseconds / 1_000) as i64),
    );
    let mut sockets = SocketSet::new(vec![]);
    let dhcp_handle = sockets.add(dhcpv4::Socket::new());
    println!("AIC8800_DHCP_STARTED");
    let mut next_diagnostic_nanoseconds = start_nanoseconds + 5_000_000_000;

    loop {
        let now_nanoseconds = axhal::time::wall_time_nanos();
        let timestamp = Instant::from_micros_const((now_nanoseconds / 1_000) as i64);
        network_interface.poll(timestamp, &mut network_device, &mut sockets);
        if network_device.transport_failed() {
            println!("AIC8800_DHCP_FAILED transport");
            return;
        }

        match sockets.get_mut::<dhcpv4::Socket>(dhcp_handle).poll() {
            Some(dhcpv4::Event::Configured(config)) => {
                network_interface.update_ip_addrs(|addresses| {
                    addresses.clear();
                    addresses.push(IpCidr::Ipv4(config.address)).unwrap();
                });
                if let Some(router) = config.router {
                    network_interface
                        .routes_mut()
                        .add_default_ipv4_route(router)
                        .unwrap();
                }
                println!(
                    "AIC8800_DHCP_PASS address={} router={:?} dns={:?}",
                    config.address, config.router, config.dns_servers
                );
                return;
            }
            Some(dhcpv4::Event::Deconfigured) => {
                network_interface.update_ip_addrs(|addresses| addresses.clear());
                network_interface.routes_mut().remove_default_ipv4_route();
            }
            None => {}
        }

        if now_nanoseconds >= next_diagnostic_nanoseconds {
            let (tx, rx, events, tx_type, tx_length, rx_type, rx_length) =
                network_device.diagnostics();
            println!(
                "AIC8800_DHCP_DIAGNOSTIC tx={} rx={} transport={} last-tx-type={:#06x} last-tx-length={} last-rx-type={:#06x} last-rx-length={}",
                tx, rx, events, tx_type, tx_length, rx_type, rx_length
            );
            next_diagnostic_nanoseconds = now_nanoseconds + 5_000_000_000;
        }

        if now_nanoseconds.saturating_sub(start_nanoseconds) >= 120_000_000_000 {
            println!("AIC8800_DHCP_FAILED timeout");
            return;
        }
        core::hint::spin_loop();
    }
}
