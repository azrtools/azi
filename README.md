# azi

[![Build Status Appveyor](https://img.shields.io/appveyor/ci/pascalgn/azi.svg?style=flat-square&label=appveyor)](https://ci.appveyor.com/project/pascalgn/azi)
[![Build Status CircleCI](https://img.shields.io/circleci/project/azrtools/azi.svg?style=flat-square&label=circleci)](https://circleci.com/gh/azrtools/azi)
[![License](https://img.shields.io/github/license/azrtools/azi.svg?style=flat-square)](LICENSE)

Show Azure information.

<img width="600" src="https://azrtools.github.io/azi/demo.svg">

## Installation

You can download binaries from the [latest release](https://github.com/azrtools/azi/releases/latest).
If you have Cargo installed, you can use `cargo install azi`

You can also use the Docker image: `docker run --rm azrtools/azi --help`

## Usage

List all subscriptions and resource groups:

```sh
azi list
```

Show Kubernetes clusters and deployments:

```sh
azi clusters -r
```

Show the costs of March 2019:

```sh
azi costs 201903
```

Show DNS entries and resource groups they point to:

```sh
azi domains
```

## Docker

To simply run the command, use `docker run --rm azrtools/azi`.
If you want to keep the authentication tokens between runs, use

```sh
docker volume create azi
docker run --rm -v azi:/home/azi azrtools/azi list
docker run --rm -v azi:/home/azi azrtools/azi dns
docker volume rm azi
```

The images are available on [Docker Hub](https://hub.docker.com/r/azrtools/azi).

## Helpful links

- https://docs.microsoft.com/en-us/rest/api/azure/
- https://docs.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-device-code
- https://docs.microsoft.com/en-us/azure/active-directory/develop/v1-protocols-oauth-code

## License

[Apache-2.0](LICENSE)
