build_dir := "build"
kernel_source_dir := "seL4"
kernel_build_dir := build_dir/"kernel/build"
kernel_install_dir := build_dir/"kernel/install"
kernel_settings := "kernel-settings.cmake"
cross_compiler_prefix := "aarch64-linux-gnu-"

clean:
    rm -rf {{build_dir}}

# kernel
alias c := configure-kernel
configure-kernel:
    cmake \
        -DCROSS_COMPILER_PREFIX={{cross_compiler_prefix}} \
        -DCMAKE_TOOLCHAIN_FILE=gcc.cmake \
        -DCMAKE_INSTALL_PREFIX={{kernel_install_dir}} \
        -C {{kernel_settings}} \
        -G Ninja \
        -S {{kernel_source_dir}} \
        -B {{kernel_build_dir}}

build-kernel: configure-kernel
    ninja -C {{kernel_build_dir}} all

install-kernel: build-kernel
    ninja -C {{kernel_build_dir}} install
    install -D -T {{kernel_build_dir}}/gen_config/kernel/gen_config.json {{kernel_install_dir}}/support/config.json
    install -D -T {{kernel_build_dir}}/kernel.dtb {{kernel_install_dir}}/support/kernel.dtb
    install -D -T {{kernel_build_dir}}/gen_headers/plat/machine/platform_gen.yaml {{kernel_install_dir}}/support/platform-info.yaml

# userspace
rust_target_path := absolute_path("support/targets")
rust_sel4_target := "aarch64-sel4"
target_dir := absolute_path(build_dir/"target")
cargo_root_dir := build_dir/"cargo-root"

build_std_options := "-Z build-std=core,alloc,compiler_builtins -Z build-std-features=compiler-builtins-mem"

common_env := "RUST_TARGET_PATH="+rust_target_path+" SEL4_PREFIX="+absolute_path(kernel_install_dir)
common_options := "--locked -Z unstable-options "+build_std_options+" --target-dir "+target_dir

app_crate := "sos"

app:
	{{common_env}} cargo build {{common_options}} -p {{app_crate}} --target {{rust_sel4_target}} --out-dir {{build_dir}}

loader_crate := "loader"
loader := cargo_root_dir/"bin"/loader_crate
loader_intermediate := build_dir/"loader.intermediate"
loader_config := absolute_path("loader-config.json")
rust_bare_metal_target := "aarch64-unknown-none"
app := absolute_path(build_dir/app_crate+".elf")
remote_options := "--git https://gitlab.com/coliasgroup/rust-seL4 --rev 7240d83b79ff8263e42ee0fd66a15189825dac99"

loader: app
	{{common_env}} \
	CC={{cross_compiler_prefix}}gcc \
	SEL4_APP={{app}} \
	SEL4_LOADER_CONFIG={{loader_config}} \
		cargo install \
		{{common_options}} \
		loader \
		{{remote_options}} \
		--target {{rust_bare_metal_target}} \
		--root {{absolute_path(cargo_root_dir)}} \
		--force \
		-Z bindeps

alias r := run
run: loader 
	qemu-system-aarch64 \
	-machine virt,virtualization=on \
	-cpu cortex-a57 -smp 4 -m 1024 \
	-nographic -serial mon:stdio \
	-kernel {{loader}}
