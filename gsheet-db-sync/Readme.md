# GSheet DB Sync

An application to sync data from google sheets into a postgres/sqlite database.

## How to use?

1. Start the program by running `cargo run --release`
2. Visit `127.0.0.1:36799`
3. Enter the Spreadsheet ID and click fetch
4. Wait till the table data is fetched
5. Fill the Database URI and click sync

## Notes

1. Each user's request spawns a seperate future and therefore runs on its own thread
