# CENNZnet Node

CENNZnet node based on Substrate

## Development Environment

### Linux and Mac

For Unix-based operating systems, you should run the following commands:

```bash
curl https://sh.rustup.rs -sSf | sh

rustup update nightly
rustup target add wasm32-unknown-unknown --toolchain nightly
rustup update stable
cargo install --git https://github.com/alexcrichton/wasm-gc
```

You will also need to install the following packages:

__Linux:__
```bash
sudo apt install cmake pkg-config libssl-dev git clang libclang-dev
```

__Mac:__
```bash
brew install cmake pkg-config openssl git llvm
```

### Windows

If you are trying to set up Substrate on Windows, you should do the following:

1. First, you will need to download and install "Build Tools for Visual Studio:"

    * You can get it at this link: https://aka.ms/buildtools
    * Run the installation file: `vs_buildtools.exe`
    * Please ensure the Windows 10 SDK component is included when installing the Visual C++ Build Tools.
    * ![image](https://i.imgur.com/zayVLmu.png)
    * Restart your computer.

2. Next, you need to install Rust:

    * Detailed instructions are provided by the [Rust Book](https://doc.rust-lang.org/book/ch01-01-installation.html#installing-rustup-on-windows).
        * Download from: https://www.rust-lang.org/tools/install
        * Run the installation file: `rustup-init.exe`
        > Note that it should not prompt you to install vs_buildtools since you did it in step 1.
        * Choose "Default Installation."
        * To get started, you need Cargo's bin directory (%USERPROFILE%\.cargo\bin) in your PATH environment variable. Future applications will automatically have the correct environment, but you may need to restart your current shell.

3. Then, you will need to run some commands in CMD to set up your Wasm Build Environment:

```bash
rustup update nightly
rustup update stable
rustup target add wasm32-unknown-unknown --toolchain nightly
```

4. Next, you install wasm-gc, which is used to slim down Wasm files:

```bash
cargo install --git https://github.com/alexcrichton/wasm-gc --force
```

5. Then, you need to install LLVM: https://releases.llvm.org/download.html

6. Next, you need to install OpenSSL, which we will do with `vcpkg`:

```
mkdir \Tools
cd \Tools
git clone https://github.com/Microsoft/vcpkg.git
cd vcpkg
.\bootstrap-vcpkg.bat
.\vcpkg.exe install openssl:x64-windows-static
```

7. After, you need to add OpenSSL to your System Variables:

```
$env:OPENSSL_DIR = 'C:\Tools\vcpkg\installed\x64-windows-static'
$env:OPENSSL_STATIC = 'Yes'
[System.Environment]::SetEnvironmentVariable('OPENSSL_DIR', $env:OPENSSL_DIR, [System.EnvironmentVariableTarget]::User)
[System.Environment]::SetEnvironmentVariable('OPENSSL_STATIC', $env:OPENSSL_STATIC, [System.EnvironmentVariableTarget]::User)
```

8. Finally, you need to install `cmake`: https://cmake.org/download/

## Development

__Build__

```bash
# compile runtime to wasm
./scripts/build.sh

# compile the node
cargo build
```


__Run__
```bash
# Run your own testnet with a validator
cargo run -- --dev
# or
./target/debug/cennznet --dev
```


__Purge chain__
```bash
# For local testnet
cargo run -- purge-chain --dev
# or
./target/debug/cennznet purge-chain --dev
```

