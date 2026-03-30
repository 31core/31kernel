TYPE=debug
ARCH=riscv64

ifeq ($(ARCH), riscv64)
	RUST_TARGET=riscv64gc-unknown-none-elf
	QEMU=qemu-system-riscv64
	QEMU_CPU=rv64
else ifeq ($(ARCH), arm64)
	RUST_TARGET=aarch64-unknown-none
	QEMU=qemu-system-aarch64
	QEMU_CPU=cortex-a57
endif

ifeq ($(TYPE), release)
	RUST_FLAGS=--release
endif

RUST_TARGET_PATH=./target/$(RUST_TARGET)/$(TYPE)
OUT=$(RUST_TARGET_PATH)/kernel

MACH=virt
CPUS=1
MEM=128M

all:
	@cargo rustc --target $(RUST_TARGET) $(RUST_FLAGS) -- -Clink-arg=-Tsrc/lds/virt.lds

run: all
	@$(QEMU) -M $(MACH) -cpu $(QEMU_CPU) -smp $(CPUS) -m $(MEM) -nographic -serial mon:stdio -bios none -kernel $(OUT)

debug: all
	@$(QEMU) -M $(MACH) -cpu $(QEMU_CPU) -smp $(CPUS) -m $(MEM) -nographic -serial mon:stdio -bios none -kernel $(OUT) -S -s

clean:
	@cargo clean
	@rm Cargo.lock
