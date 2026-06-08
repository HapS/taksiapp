#!/bin/bash

#!/bin/bash
# Versiyon artırmak için: ./scripts/bump_version.sh patch|minor|major

PART=${1:-patch}

# Cargo.toml'dan mevcut versiyonu al
CURRENT=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

# Versiyonu parçala
IFS='.' read -ra VERSION <<< "$CURRENT"
MAJOR=${VERSION[0]}
MINOR=${VERSION[1]}
PATCH=${VERSION[2]}

# Versiyonu artır
case $PART in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
    *)
        echo "Kullanım: $0 patch|minor|major"
        exit 1
        ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"

# Cargo.toml'ı güncelle
sed -i '' "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" Cargo.toml

echo "Versiyon: $CURRENT -> $NEW_VERSION"




cargo build --release --target x86_64-unknown-linux-musl
