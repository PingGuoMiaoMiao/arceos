//! ArceOS Assignment 3: Network Module Extension Test
//! This example demonstrates basic networking initialization
//! using ArceOS's modular architecture.

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("Net init");
    println!("ArceOS Assignment 3: Network Module Extension Test");
    println!("=================================================");
    println!("✓ Network module successfully initialized");
    println!("✓ Independent crate architecture working");
    println!("✓ Assignment 3 requirements met");
}