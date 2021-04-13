pipeline {
    agent {
        docker {
            label 'benchmark'
            image 'rustlang/rust:nightly'
            args '-u root:root'
        }
    }
    environment {
        RUST_VERSION='1.50.0'
        RUST_NIGHTLY='nightly-2021-02-21'
    }
    stages {
        stage('Prepare') {
            steps {
		sh 'lscpu'
		sh 'free -h'
                sh '''
                  apt-get update && \
                  apt-get -y install apt-utils cmake pkg-config libssl-dev git clang libclang-dev && \
                  rustup uninstall nightly && \
                  rustup install $RUST_VERSION && \
                  rustup install $RUST_NIGHTLY && \
                  rustup default $RUST_VERSION && \
                  rustup target add --toolchain $RUST_NIGHTLY wasm32-unknown-unknown && \
                  rustup target add --toolchain $RUST_VERSION x86_64-unknown-linux-musl && \
                  mv /usr/local/rustup/toolchains/nightly* /usr/local/rustup/toolchains/nightly-x86_64-unknown-linux-gnu
                '''
            }
        }

        stage('Build with extra features') {
            steps{
                sh 'cargo build --release --features runtime-benchmarks'
            }
        }

        stage('Run Benchmarks') {
            steps{
                sh 'cargo test -p crml-staking --features runtime-benchmarks'
                sh 'cargo test -p crml-cennzx --features runtime-benchmarks'
                sh './target/release/cennznet benchmark --chain dev --steps 50 --repeat 2 --pallet "*" --extrinsic "*" --raw --execution=wasm --wasm-execution=compiled --output'
                archiveArtifacts artifacts: '*.rs'
            }
        }
    }
}
