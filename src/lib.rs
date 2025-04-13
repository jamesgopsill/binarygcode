#![doc = include_str!("../README.md")]
#![no_std]

#[macro_use]
extern crate alloc;

pub mod common;
pub mod deserialiser;

#[cfg(test)]
mod tests;
