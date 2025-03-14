# ODBCSV

Query an ODBC data source and output the result as CSV or to insert from CSV into an ODBC data source.

## Installation

Several installation options are available.

### Download prebuilt binaries

You can download the latest binaries here from the GitHub release: <https://github.com/pacman82/odbcsv/releases/latest>.

### Install using cargo

```shell
cargo install odbcsv
```

## Usage

### Querying an Microsoft SQL Database and storing the result in a file

```shell
odbcsv query \
--output query.csv \
--connection-string "Driver={ODBC Driver 18 for SQL Server};Server=localhost;UID=SA;PWD=My@Test@Password1;" \
"SELECT title, year from Movies"
```

### Specify user and password independently

```shell
odbcsv query \
--output query.csv \
--connection-string "Driver={ODBC Driver 18 for SQL Server};Server=localhost;" \
--user "SA" \
--password "My@Test@Password1" \
"SELECT title, year from Movies"
```

Alternatively you may also specify the `ODBC_USER` and `ODBC_PASSWORD` environment variable.

### Query using data source name

```bash
odbcsv query \
--output query.csv \
--dsn my_db \
--password "My@Test@Password1" \
--user "SA" \
"SELECT * FROM Birthdays"
```

### Use parameters in query

```shell
odbcsv query \
--output query.csv \
--connection-string "Driver={ODBC Driver 18 for SQL Server};Server=localhost;UID=SA;PWD=My@Test@Password1;" \
"SELECT * FROM Birthdays WHERE year > ? and year < ?" \
1990 2010
```

### Insert data from CSV to database

```shell
odbcsv insert \
--input birthdays.csv \
--connection-string "Driver={ODBC Driver 18 for SQL Server};Server=localhost;UID=SA;PWD=My@Test@Password1;" \
Birthdays \
```

Use `--help` to see all options.
