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
		sh 'git clean -fdx'
		sh 'mkdir clean_dir && chmod 777 clean_dir'
		dir('clean_dir'){
		    checkout([$class: 'GitSCM', branches: [[name: '${CHANGE_BRANCH}']], extensions: [], userRemoteConfigs: [[credentialsId: 'cennznet-bot-ssh-key', url: 'git@github.com:cennznet/cennznet.git']]])
		    sh 'ls -l'
		    sh 'git branch'
		    sh 'git branch -a'
		    sh 'git checkout ${CHANGE_BRANCH}'
		    sh 'echo HELLO >> hello.txt'
		    sh 'git config --global user.email "devops@centrality.ai" && git config --global user.name "cennznet-bot"'
		    withCredentials([sshUserPrivateKey(credentialsId: "cennznet-bot-ssh-key", keyFileVariable: 'keyfile')]) {
			sh 'mkdir -p ~/.ssh/'
			sh 'cp ${keyfile} ~/.ssh/id_rsa'
			sh 'ls ~/.ssh/'
			sh 'ssh-keyscan -t rsa github.com >> ~/.ssh/known_hosts'
			sh 'echo HELLOWORLD >> a.txt'
			sh 'git add a.txt; git commit -m "add a.txt"; git push'
		    }
		}

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
                sh './target/release/cennznet benchmark --chain dev --steps 50 --repeat 2 --pallet "*" --extrinsic "*" --raw --execution=wasm --wasm-execution=compiled'
		sh 'touch empty.rs'
                archiveArtifacts artifacts: '*.rs'
            }
        }

	stage('Commit files back') {
	    steps {
		sh 'git checkout ${CHANGE_BRANCH}'
		sh 'git branch -a'
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
