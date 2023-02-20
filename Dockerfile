FROM rust:slim as builder

COPY . /home/

WORKDIR /home/gsheet-db-sync
RUN cargo build --release

WORKDIR /home/csv-parse
RUN cargo build --release

FROM debian:bullseye-slim as runner
COPY --from=builder /home/gsheet-db-sync/target/release/gsheet-db-sync /home
COPY --from=builder /home/csv-parse/target/release/csv-parse /home
WORKDIR /home
CMD ./gsheet-db-sync & ./csv-parse