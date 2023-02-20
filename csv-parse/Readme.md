# CSV Parser

Allows a user to upload a CSV file and then displays that file as an html table.
It does this by converting the CSV file to some JSON internally.
It sends the uploaded CSV file to the `/csv_to_json` endpoint to parse the csv file as json.

## How to use?

1. Start the program by running `cargo run --release`
2. Visit `127.0.0.1:8888`
3. Click on the `Browse` button to upload a file.
4. Select and upload a file.
5. Click on `Submit` to see the file as an html table.
