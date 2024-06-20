# Viam Micro-RDK Module Template

## (In)stability Notice

> **Warning** The Viam Micro-RDK is currently in beta.

## Overview

This repository defines a template for use with
[`cargo-generate`](https://cargo-generate.github.io/cargo-generate). Projects
created from this template are a starting point for defining modular
resources for the Micro-RDK.

Modular resources for the Viam Micro-RDK function differently than
modular resources for the standard Viam RDK, because they must be
compiled into the image which will be flashed to the microcontroller.

## Prerequisites

The modules produced using this repository are designed to
interoperate with projects developed by following the [Micro-RDK
Development
Setup](https://docs.viam.com/installation/prepare/microcontrollers/development-setup)
process. Please ensure that your environment is properly configured
per those instructions before experimenting with creating Micro-RDK
modules.

## Usage

Run the following command to create a new git repository for your
module (you will be promted for the name) in the current directory:

`cargo generate --git https://github.com/viamrobotics/micro-rdk templates/module`

Answer the prompts (project name, target architecture, etc.), and then
implement your modular resources within the newly formed project. The
`register_models` function defined in `src/lib.rs` will be called for
you when your robot project starts up, and you can register your newly
defined models with the Micro-RDK by invoking the appropriate methods
on the Micro-RDK `ComponentRegistry` argument.

Once you have implemented your module, you can use it in your
Micro-RDK robot project simply by adding it as a standard dependency
in the `[dependencies]` section of your robot project's `Cargo.toml`
file. The `register_models` entry point of all dependencies produced
by this template will be automatically invoked at startup.

## Tutorial

Please see the [Modular Driver
Examples](https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers)
to see some example Micro-RDK module projects. The
[README](https://github.com/viamrobotics/micro-rdk/blob/main/examples/modular-drivers/README.md)
provides a walkthrough of how to implement a module for the Micro-RDK.

## Caveats

Module auto-registration is a protocol between this template and the
[Micro-RDK Robot
Template](https://github.com/viamrobotics/micro-rdk-robot-template);
the Micro-RDK itself does not directly participate. If you create a
Micro-RDK project from scratch (i.e. it was not created by the robot
template), then module auto-registration will not happen and your
project must manually invoke the `register_models` entry point for all
crates that are intended to offer modular resources.
