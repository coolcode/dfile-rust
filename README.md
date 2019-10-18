# DFile: A fancy S3-based file sharing mode [Rust + Rocket]

[![Codacy Badge](https://api.codacy.com/project/badge/Grade/3b25d03f9535456997878815286921eb)](https://www.codacy.com/manual/coolcode/dfile?utm_source=github.com&utm_medium=referral&utm_content=coolcode/dfile&utm_campaign=Badge_Grade)

This is a no-bullshit and S3-based file hosting that also runs on [https://dfile.app](https://dfile.app)

![img](https://github.com/coolcode/dfile/blob/master/share/img/dfile.png?raw=true)

## DFile backend (server)

Before running the service for the first time, run

```bash
cp sample.env .env
```

Modify .env (mainly setup your S3 settings)

```bash

```

Run it

```bash
cargo run
```

## DFile frontend (app)

Install yarn first: [https://yarnpkg.com/lang/en/docs/install/](https://yarnpkg.com/lang/en/docs/install/)

```bash
# run
yarn
yarn dev

# export to production
yarn export
```

## How to use

```bash
# Upload using cURL
➜ curl -F file=@yourfile.txt http://localhost:8000
http://localhost:8000/QmV...HZ
# Download the file
➜ curl -L http://localhost:8000/QmV...HZ -o yourfile.txt
```
