#!/bin/bash

CONFIG_FILE=$1

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Configuration file not found"
    exit 0
fi

PROJECT_NAME=$(cat ${CONFIG_FILE} | jq -r '.default_project')
PROJECT_PATH=$(dirname "$(realpath ${CONFIG_FILE})")
IMG_PATH="${PROJECT_PATH}/${PROJECT_NAME}/btrfs.img"
MOUNT_POINT="/mnt/dbranch/${PROJECT_NAME}"
IMG_SIZE=$((1 * 1024 * 1024 * 1024 * 1024))  # 1TB em bytes
USER_ID=$(id -u)

# output colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== BTRFS Initialization Script ===${NC}"
echo "Project: ${PROJECT_NAME}"
echo "Image path: ${IMG_PATH}"
echo "Mount point: ${MOUNT_POINT}"
echo ""

# ========================================
# 1. Verificar instalação do BTRFS
# ========================================
echo -e "${YELLOW}Checking BTRFS installation...${NC}"
if ! command -v btrfs &> /dev/null; then
    echo -e "${RED}ERROR: BTRFS is not installed!${NC}"
    echo "Install with: sudo apt-get install btrfs-progs  # Debian/Ubuntu"
    echo "            sudo dnf install btrfs-progs      # Fedora"
    echo "            sudo pacman -S btrfs-progs      # Arch"
    exit 1
fi

btrfs version
echo -e "${GREEN}✓ BTRFS installed${NC}"
echo ""

echo "Creating project directory: $(dirname ${IMG_PATH})"
mkdir -p "$(dirname ${IMG_PATH})"

if [ -f "${IMG_PATH}" ]; then
    echo -e "${YELLOW}Image file already exists ${NC}"
    # echo "Aborting..."
    # exit 0
fi

echo -e "${YELLOW}Mounting BTRFS disk...${NC}"

# Create loop device
echo "Creating loop device for image..."
LOOP_DEVICE=$(sudo losetup -f --show "${IMG_PATH}")

if [ -z "${LOOP_DEVICE}" ]; then
    echo -e "${RED}ERROR: Failed to create loop device${NC}"
    exit 1
fi

echo "Loop device created: ${LOOP_DEVICE}"

# Format as BTRFS
echo "Formatting ${LOOP_DEVICE} as BTRFS..."
sudo mkfs.btrfs -f "${LOOP_DEVICE}"

if [ $? -ne 0 ]; then
    echo -e "${RED}ERROR: Failed to format device as BTRFS${NC}"
    sudo losetup -d "${LOOP_DEVICE}"
    exit 1
fi

echo -e "${GREEN}✓ Device formatted as BTRFS${NC}"

# Create mount point
echo "Creating mount point: ${MOUNT_POINT}"
sudo mkdir -p "${MOUNT_POINT}"
sudo setfacl -R -m u:${USER_ID}:rwx "${MOUNT_POINT}"

# Mount the device
echo "Mounting ${LOOP_DEVICE} at ${MOUNT_POINT}..."
sudo mount "${LOOP_DEVICE}" "${MOUNT_POINT}"

if [ $? -ne 0 ]; then
    echo -e "${RED}ERROR: Failed to mount device${NC}"
    sudo losetup -d "${LOOP_DEVICE}"
    exit 1
fi

echo -e "${GREEN}✓ Device mounted at ${MOUNT_POINT}${NC}"

# Create main subvolume
echo "Creating main subvolume..."
sudo btrfs subvolume create "${MOUNT_POINT}/main"
sudo setfacl -m u:${USER_ID}:rwx "${MOUNT_POINT}/main"

if [ $? -ne 0 ]; then
    echo -e "${RED}ERROR: Failed to create main subvolume${NC}"
    sudo umount "${MOUNT_POINT}"
    sudo losetup -d "${LOOP_DEVICE}"
    exit 1
fi

echo -e "${GREEN}✓ Subvolume 'main' created${NC}"

# Create data directory in main subvolume
echo "Creating data directory..."
mkdir -p "${MOUNT_POINT}/main/data/pgdata"
sudo chown -R ${USER_ID}:${USER_ID} "${MOUNT_POINT}"

echo -e "${GREEN}✓ Directory structure created${NC}"
echo ""

