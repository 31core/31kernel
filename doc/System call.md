# System call ABI

## Register usage
### RISC-V64

| Register | Purpose       |
|----------|-------------- |
| `a7`     | Syscall number|
| `a0`     | Argument 0    |
| `a1`     | Argument 1    |
| `a2`     | Argument 2    |
| `a3`     | Argument 3    |
| `a0`     | Return value  |

### ARM64

| Register | Purpose       |
|----------|---------------|
| `x8`     | Syscall number|
| `x0`     | Argument 0    |
| `x1`     | Argument 1    |
| `x2`     | Argument 2    |
| `x3`     | Argument 3    |
| `x0`     | Return value  |

## System call table

| ID   | Name  | Argument 0 | Argument 1 | Argument 2 | Argument 3 | Return value |
|------|-------|------------|------------|------------|------------|--------------|
| 0    | exit  | Exit code  | -          | -          | -          | -            |
| 1    | open  | Path string pointer     | -          | -          | -            | File descriptor, `-1` for any error. |
| 3    | write | File descriptor         | Buffer pointer          | Length of buffer | -            | Length of written bytes, `-1` for any error.          |
| 6    | sleep | Timestamp  in nanosecond| -          | -          | -            |-           |
| 7    | fork  | -          | -          | -          | -          | Child PID for parent process, `0` for child process|
