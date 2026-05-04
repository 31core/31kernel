# 31kernel

31kernel is a generic kernel written in Rust.

## Supported platforms

|Indentifier|Rust target|
|-----------|-----------|
|arm64      |aarch64-unknown-none|
|riscv64    |riscv64gc-unknown-none-elf|

## Source tree structure

|Directory |Description|
|----------|-----------|
|crypto    |Cryptographic algorithm implementations.|
|doc       |Documentations.|
|lib       |Common libraries.   |
|src/arch  |Architecture-specific code.|
|src/device|Device drivers.|
