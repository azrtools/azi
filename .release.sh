#!/bin/bash

RELEASE="$1"

if [[ -z "${RELEASE}" ]]; then
    echo "Usage: $0 <release>" >&2
    exit 1
fi

if [[ -z "${GITHUB_TOKEN}" ]]; then
    echo "Missing GITHUB_TOKEN" >&2
    exit 1
fi

REPO="azrtools/azi"

mkdir -p target

# Linux binary
CIRCLECI_URL=$(curl "https://circleci.com/api/v1.1/project/github/${REPO}/latest/artifacts?branch=master" | jq -r .[0].url)
curl -L "${CIRCLECI_URL}" > target/azi-linux64

# Windows binary
APPVEYOR_BUILD_ID=$(curl -s -H "Accept: application/json" https://ci.appveyor.com/api/projects/${REPO}/branch/master | jq -r .build.jobs[0].jobId)
curl -L "https://ci.appveyor.com/api/buildjobs/${APPVEYOR_BUILD_ID}/artifacts/target/release/azi.exe" > target/azi-win64.exe

# MacOS binary
make release || exit 1
cp target/release/azi target/azi-macos-amd64

# Upload artifacts:

GH_RELEASE_ID=$(curl -s "https://api.github.com/repos/${REPO}/releases/tags/${RELEASE}" | jq -r .id)

if [[ -z "${GH_RELEASE_ID}" || "${GH_RELEASE_ID}" == "null" ]]; then
    echo "Release not found: ${RELEASE}" >&2
    exit 2
fi

GH_UPLOAD_URL="https://uploads.github.com/repos/${REPO}/releases/${GH_RELEASE_ID}/assets"

for f in "azi-macos-amd64" "azi-linux64" "azi-win64.exe"; do
    echo "Uploading ${f}"
    CONTENT_TYPE="$(file -b --mime-type target/${f})"
    curl -H "Authorization: token ${GITHUB_TOKEN}" -H "Content-Type: ${CONTENT_TYPE}" \
        --data-binary "@target/${f}" "${GH_UPLOAD_URL}?name=${f}" >/dev/null || exit 1
done
