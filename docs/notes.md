# Project Notes

## Initial Setup

- Github pre-generated `README.md` and `.gitignore` files
- VSCodium with `rust-analyzer` plugin

## Coding

- Reference:
    - docs.rs
    - https://rust-lang-nursery.github.io/rust-cookbook
- Started with `main.rs` in order to test the structs, serializing, deserializing, and bringing in csv
- Separated out the code so it would be easier to maintain
- Iterated on each transaction type
- Tested with small test CSV
- Used AI to help build the robust testing for both happy path and non-happy path
- Used AI to perform a code review
- Itereated on AI response to the code review and called for additional reviews

## Use of AI

## Setting Up Tests

- `Read through the functions defined in client.rs closely and the structure setup for tests. Write test cases for all of the functions, both happy and unhappy paths. Use parametrize to test multiple clients and amounts for testing`
- `Read through the functions defined in bankrecords.rs closely and the structure setup for tests. Write test cases for all of the functions, both happy and unhappy paths. Use parametrize to test multiple paths for each function`
- `Read through the functions defined in processor.rs closely and the structure setup for tests. Write test cases for all of the functions, both happy and unhappy paths. Use parametrize to test multiple paths for each function`
- `Looking at the spec, write a robust transactions.csv and accounts.csv for testing against`
- `Based on the spec, review the logic of the code to verify that all requirements have been met`
- `/custom-review`

# Additional Notes

- Storage
    - Toy project: Kept everything in memory. This will be a one shot with a small amount of data, so we don't need more robust storage
    - Further toy project: For further development, we could dump the finished transactions and clients to csv file to be read at startup
    - Production: Use a database, perferably a high performance one
- Error Messages
    - Errors are simply dumped to `stderr` more robust error logs should be used
    - Additional in memory (toy) or db (production) tables should be used to track errors
    - Critical errors and client locking should cause alerts
- Github Actions
    - By setting up github actions we can verify all tests pass appropriately