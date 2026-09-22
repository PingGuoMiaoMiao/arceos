extern crate alloc;

use axmodel_mushroom_yolov5::{
    Detection, MODEL_INPUT_LENGTH, MemoryLayout, ModelOutputError, SourceImageMeta,
    decode_model_outputs, intersection_over_union, map_detection_to_source,
    non_maximum_suppression, quantize_rgb_u8_in_place,
};

#[test]
fn quantization_matches_the_fixed_sdk_formula() {
    let mut pixels = [0, 1, 2, 127, 128, 254, 255];
    quantize_rgb_u8_in_place(&mut pixels);
    assert_eq!(pixels, [0, 0, 1, 63, 64, 127, 127]);
}

#[test]
fn fixed_model_input_is_three_640_square_planes() {
    assert_eq!(MODEL_INPUT_LENGTH, 3 * 640 * 640);
}

#[test]
fn maps_model_coordinates_back_to_the_source_image() {
    let meta = SourceImageMeta {
        source_width: 1280,
        source_height: 720,
        resized_width: 640,
        resized_height: 360,
        pad_x: 0,
        pad_y: 140,
    };
    let mapped = map_detection_to_source(
        Detection {
            x1: 100.0,
            y1: 150.0,
            x2: 200.0,
            y2: 250.0,
            confidence: 0.9,
        },
        meta,
    );
    assert_eq!(mapped.x1, 200.0);
    assert_eq!(mapped.y1, 20.0);
    assert_eq!(mapped.x2, 400.0);
    assert_eq!(mapped.y2, 220.0);
    assert_eq!(mapped.confidence, 0.9);
}

#[test]
fn coordinate_mapping_clamps_padding_and_model_overflow() {
    let meta = SourceImageMeta {
        source_width: 720,
        source_height: 1280,
        resized_width: 360,
        resized_height: 640,
        pad_x: 140,
        pad_y: 0,
    };
    let mapped = map_detection_to_source(
        Detection {
            x1: 0.0,
            y1: -10.0,
            x2: 700.0,
            y2: 650.0,
            confidence: 0.5,
        },
        meta,
    );
    assert_eq!(
        (mapped.x1, mapped.y1, mapped.x2, mapped.y2),
        (0.0, 0.0, 720.0, 1280.0)
    );
}

#[test]
fn calculates_intersection_over_union() {
    let a = Detection {
        x1: 0.0,
        y1: 0.0,
        x2: 10.0,
        y2: 10.0,
        confidence: 0.9,
    };
    let b = Detection {
        x1: 5.0,
        y1: 5.0,
        x2: 15.0,
        y2: 15.0,
        confidence: 0.8,
    };
    let actual = intersection_over_union(a, b);
    assert!((actual - 25.0 / 175.0).abs() < 1.0e-6);
}

#[test]
fn nms_keeps_the_highest_confidence_overlapping_box() {
    let detections = alloc::vec![
        Detection {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
            confidence: 0.8
        },
        Detection {
            x1: 1.0,
            y1: 1.0,
            x2: 99.0,
            y2: 99.0,
            confidence: 0.9
        },
        Detection {
            x1: 200.0,
            y1: 200.0,
            x2: 220.0,
            y2: 220.0,
            confidence: 0.7
        },
    ];
    let kept = non_maximum_suppression(detections);
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].confidence, 0.9);
    assert_eq!(kept[1].confidence, 0.7);
}

#[test]
fn memory_layout_matches_the_verified_model_artifacts() {
    let layout = MemoryLayout::new(783_488, 7_108_528).unwrap();
    assert_eq!(layout.dmabuf, 0);
    assert_eq!(layout.weight, 786_432);
    assert_eq!(layout.shared, 7_897_088);
    assert_eq!(layout.private, 13_631_488);
    assert_eq!(layout.input_fp32, 14_860_288);
    assert_eq!(layout.output_20, 19_775_488);
    assert_eq!(layout.output_80, 19_808_256);
    assert_eq!(layout.output_40, 20_271_104);
    assert_eq!(layout.used, 20_389_888);
}

#[test]
fn output_decoder_rejects_wrong_tensor_lengths() {
    assert_eq!(
        decode_model_outputs(&[], &[], &[]),
        Err(ModelOutputError::OutputLength {
            output: "output_80",
            expected: 115_200,
            actual: 0,
        })
    );
}

#[test]
fn output_decoder_returns_no_boxes_for_very_negative_logits() {
    let output_80 = alloc::vec![-100.0; 115_200];
    let output_40 = alloc::vec![-100.0; 28_800];
    let output_20 = alloc::vec![-100.0; 7_200];
    assert!(
        decode_model_outputs(&output_80, &output_40, &output_20)
            .unwrap()
            .is_empty()
    );
}
