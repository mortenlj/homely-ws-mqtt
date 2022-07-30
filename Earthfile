VERSION 0.6

FROM rust:1.61

WORKDIR /code

# Constants, do not override
ARG cross_version=0.2.1
ARG CARGO_INCREMENTAL=0

prepare:
    DO github.com/earthly/lib+INSTALL_DIND
    RUN cargo install cross --version ${cross_version}
    COPY --dir src Cargo.lock Cargo.toml .
    SAVE IMAGE --cache-hint

build:
    FROM +prepare

    ARG target_platform
    ARG version=unknown

    IF [ ${target_platform} = "linux/amd64" ]
        ARG target=x86_64-unknown-linux-gnu
    ELSE IF [ ${target_platform} = "linux/arm64" ]
        ARG target=aarch64-unknown-linux-gnu
    END

    WITH DOCKER \
        --pull rustembedded/cross:${target}-${cross_version}
        RUN cross build --target ${target} --release
    END

    SAVE ARTIFACT target/${target}/release/homely-ws-mqtt homely-ws-mqtt

docker:
    FROM gcr.io/distroless/cc

    WORKDIR /app

    ARG TARGETPLATFORM
    ARG USERPLATFORM

    COPY --platform=${USERPLATFORM} (+build/homely-ws-mqtt --target_platform=${TARGETPLATFORM}) /app/homely-ws-mqtt
    ENTRYPOINT ["/app/homely-ws-mqtt"]

    # builtins must be declared
    ARG EARTHLY_GIT_PROJECT_NAME
    ARG EARTHLY_GIT_SHORT_HASH

    # Override from command-line on CI
    ARG main_image=ghcr.io/$EARTHLY_GIT_PROJECT_NAME
    ARG version=$EARTHLY_GIT_SHORT_HASH

    SAVE IMAGE --push ${main_image}:${version} ${main_image}:latest

deploy:
    BUILD --platform=linux/amd64 --platform=linux/arm64 +docker
