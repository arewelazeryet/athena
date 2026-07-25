FROM alpine:3.22 AS runtime
ARG TARGETARCH
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY dist/${TARGETARCH}/athena /usr/local/bin/athena

CMD ["athena"]
