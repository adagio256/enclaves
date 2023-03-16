#!/bin/sh

set -e

# If running within github actions, operate within mounted FS, else operate from root
BASE_PATH=$GITHUB_WORKSPACE
OUTPUT_PATH="$BASE_PATH/output"

PACKAGES_PATH=/packages
cd $PACKAGES_PATH

### Build runit
gunzip runit-2.1.2.tar.gz
tar -xpf runit-2.1.2.tar
cd admin/runit-2.1.2 # runit contains a top level folder called admin

# compile runit
echo "****************************"
echo "* compiling runit binaries *"
echo "****************************"

# Configure static compilation of runit using dietlibc
echo 'gcc -O2 -Wall -static' >src/conf-cc
echo 'gcc -static -Os -pipe' >src/conf-ld
./package/compile
./package/check

# Create expected directories for runit
mkdir -p "$OUTPUT_PATH/runit-2.1.2/src"

# Move compiled runit commands into output commands folder
echo "************************************"
echo "* copying runit binaries to output *"
echo "************************************"
cp -r command "$OUTPUT_PATH/runit-2.1.2"

# Move compiled runit scripts into output scripts folder
cp -r ./package "$OUTPUT_PATH/runit-2.1.2"

# extract net-tools source
cd $PACKAGES_PATH
echo "************************"
echo "* extracting net-tools *"
echo "************************"
xz -d net-tools-2.10.tar.xz ; tar -xf net-tools-2.10.tar

echo "**********************"
echo "* building net-tools *"
echo "**********************"
cd net-tools-2.10
# Use preconfigured config for Cage environment
cp "$PACKAGES_PATH/net-tools.h" ./config.h


# Run make commands required for ifconfig, include static flag
CFLAGS="-O2 -g -static" make subdirs
CFLAGS="-O2 -g -static" make ifconfig

mkdir -p "$OUTPUT_PATH/net-tools-2.10"

# Copy ifconfig binary to output directory
echo "*******************************"
echo "* copying ifconfig to outputs *"
echo "*******************************"
cp ./ifconfig "$OUTPUT_PATH/net-tools-2.10"

# Create archive of static binaries and installer
echo "******************************"
echo "* creating installer archive *"
echo "******************************"
cp "$PACKAGES_PATH/installer.sh" "$OUTPUT_PATH/installer.sh"
cd $OUTPUT_PATH
tar -czf runtime-dependencies.tar.gz net-tools-2.10 runit-2.1.2 installer.sh

# Remove binaries outside of the archive
echo "*****************************"
echo "* removing unused artifacts *"
echo "*****************************"
rm -rf net-tools-2.10 runit-2.1.2 installer.sh
