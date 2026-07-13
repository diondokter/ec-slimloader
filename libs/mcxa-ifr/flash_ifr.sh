#!/bin/sh

# Generate the bin
cargo test

probe-rs ./ifr.bin --binary-format bin --base-address 0x01002000 --reset --chip-description-path ./MCXA.yaml --chip MCXA577
