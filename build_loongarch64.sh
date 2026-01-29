#!/bin/bash 
#/home/vector # cat /etc/os-release
#NAME="Alpine Linux"
#ID=alpine
#VERSION_ID=3.21.0
#PRETTY_NAME="Alpine Linux v3.21"
#HOME_URL="https://alpinelinux.org/"
#BUG_REPORT_URL="https://gitlab.alpinelinux.org/alpine/aports/-/issues"

#git checkout v0.44.0
apk add git gcc vim rust cargo make pkgconfig openssl-dev protoc g++ cyrus-sasl-dev bash perl
export OPENSSL_NO_VENDOR=1

make build

TRIPLE=0.44.0-unknown-linux-musl
PROFILE=release
mkdir -p target/${TRIPLE}
ln -s ../release target/${TRIPLE}/release
rm -rf target/scratch/vector-${TRIPLE} || true
mkdir -p target/scratch/vector-${TRIPLE}/bin target/scratch/vector-${TRIPLE}/etc
cp -R -f -v \
        target/${TRIPLE}/${PROFILE}/vector \
        target/scratch/vector-${TRIPLE}/bin/vector
cp -R -f -v \
        README.md \
        LICENSE \
        licenses \
        NOTICE \
        LICENSE-3rdparty.csv \
        config \
        target/scratch/vector-${TRIPLE}/
cp -R -f -v \
        distribution/systemd \
        target/scratch/vector-${TRIPLE}/etc/
tar --create \
        --gzip \
        --verbose \
        --file target/${TRIPLE}/${PROFILE}/vector-${TRIPLE}.tar.gz \
        --directory target/scratch/ \
        ./vector-${TRIPLE}
rm -rf target/scratch/
