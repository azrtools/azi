# azi

[![Build Status Appveyor](https://img.shields.io/appveyor/ci/pascalgn/azi.svg?style=flat-square&label=appveyor)](https://ci.appveyor.com/project/pascalgn/azi)
[![Build Status CircleCI](https://img.shields.io/circleci/project/pascalgn/azi.svg?style=flat-square&label=circleci)](https://circleci.com/gh/pascalgn/azi)
[![License](https://img.shields.io/github/license/pascalgn/azi.svg?style=flat-square)](LICENSE)

Show Azure information.

## Installation

If you have Cargo installed, you can use `cargo install azi`

## Usage

List all subscriptions and resource groups:

```
azi list
```

Show the costs of March 2019:

```
azi costs 201903
```

Show DNS entries and resource groups they point to:

```
azi domains
```

## Helpful links

- https://docs.microsoft.com/en-us/rest/api/azure/
- https://docs.microsoft.com/en-us/azure/active-directory/develop/v1-protocols-oauth-code

## License

Apache-2.0
