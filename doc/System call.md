# System call ABI

## Register Usage
### RISC-V64

|Register|Purpose       |
|--------|--------------|
|`a7`    |Syscall number|
|`a0`    |Argument 0    |
|`a1`    |Argument 1    |
|`a2`    |Argument 2    |
|`a3`    |Argument 3    |
|`a0`    |Return value  |

### ARM64

|Register|Purpose       |
|--------|--------------|
|`x8`    |Syscall number|
|`x0`    |Argument 0    |
|`x1`    |Argument 1    |
|`x2`    |Argument 2    |
|`x3`    |Argument 3    |
|`x0`    |Return value  |

## syscall table

|ID|Name|Argument 0|Argument 1|Argument 2|Argument 3|Return value|
|--|----|----------|----------|----------|----------|------------|
|0 |exit|Exit code |-         |-         |-         |-           |
