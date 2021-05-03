pipeline {
    agent none
    environment {
        RUST_VERSION='1.50.0'
        RUST_NIGHTLY='nightly-2021-02-21'
    }
    options{
        lock('singleton-build') 
    }
    stages {
        stage('Prepare and Build with extra features') {
            agent {
                docker {
                    label 'benchmark'
                    image 'rustlang/rust:nightly'
                    args '-u root:root'
                }
            }
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
                /**
                sh 'cargo build --release --features runtime-benchmarks'
                sh 'cargo test -p crml-staking --features runtime-benchmarks'
                sh 'cargo test -p crml-cennzx --features runtime-benchmarks'
                sh 'cargo test -p crml-nft --features runtime-benchmarks'
                 */
            }
        }

        stage('Run Benchmarks') {
            agent { label 'benchmark'}
            steps {
                sh 'rm -rf output_dir && mkdir output_dir'
                //sh './target/release/cennznet benchmark --chain dev --steps 50 --repeat 100 --pallet "*" --extrinsic "*" --raw --execution=wasm --wasm-execution=compiled --output output_dir'
                sh 'echo "dummy test" >> output_dir/a.txt'
                archiveArtifacts artifacts: 'output_dir/*'
            }
        }

        stage('Commit files back') {
            agent {
                docker {
                    label 'benchmark'
                    //image 'rustlang/rust:nightly'
                    image 'maochuanli/debian-buster:latest'
                    args '-u root:root'
                }
            }
            environment {
                GPG_PRIVATE_KEY = credentials('cennznet-bot-gpg-private-key')
                GPG_PUBLIC_KEY = credentials('cennznet-bot-gpg-public-key')
            }
            steps {
                sh 'mkdir clean_dir && chmod 777 clean_dir'
                dir('clean_dir'){
                    checkout([$class: 'GitSCM', branches: [[name: '${CHANGE_BRANCH}']], extensions: [], userRemoteConfigs: [[url: 'git@github.com:cennznet/cennznet.git']]])
                    sh 'git checkout ${CHANGE_BRANCH}'
                    sh 'git branch'
                    sh 'cp ../output_dir/* runtime/src/weights/'
                    sh 'git config --global user.email "devops@centrality.ai" && git config --global user.name "cennznet-bot"'
                    sh 'git config --global commit.gpgsign true'
                    withCredentials([sshUserPrivateKey(credentialsId: "cennznet-bot-ssh-key", keyFileVariable: 'keyfile')]) {
                        sh 'mkdir -p ~/.ssh/'
                        sh 'cp ${keyfile} ~/.ssh/id_rsa'
                        sh 'ls ~/.ssh/'
                        sh 'ssh-keyscan -t rsa github.com >> ~/.ssh/known_hosts'
                        sh 'git diff'
                        sh 'gpg --list-keys'
                        sh 'gpg --import ${GPG_PUBLIC_KEY}'
                        sh 'gpg --allow-secret-key-import --import ${GPG_PRIVATE_KEY}'
                        sh 'gpg --list-keys'
                        sh 'git add .; git commit -S -m "add new benchmark files `date` with gpg signature"; git push'
                    }
                }

            }
            post {
                always {
                    echo "clean workspace"
                    cleanWs()
                }
            }

        }
    }
}
