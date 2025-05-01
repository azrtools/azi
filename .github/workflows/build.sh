#!/bin/sh

set -e

repository="azrtools/azi"
platforms="linux/amd64,linux/arm64"

build_docker() {
  version="$1"
  tag="$repository:$version"
  echo "Building $tag..."
  docker buildx build --push --platform="$platforms" . -t "$tag"
}

echo "${DOCKER_PASSWORD}" |
  docker login -u "${DOCKER_USERNAME}" --password-stdin

if [ "$REF_NAME" = "main" ]; then
  build_docker "latest"
elif echo "$REF_NAME" | grep -Eq '[0-9]+\.[0-9]+\.[0-9]+'; then
  build_docker "$REF_NAME"
fi
