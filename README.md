# RNG Query

[![crates.io](https://img.shields.io/crates/v/rng-query)](https://crates.io/crates/rng-query)
[![docs.rs](https://img.shields.io/docsrs/rng-query)](https://docs.rs/rng-query/)
[![license](https://img.shields.io/crates/l/rng-query)](./LICENSE)

> CLI to use pseudorandomness the easy way and help me with decision paralysis

```sh
rq "a, b, c"              # choose a, b or c at random
rq "2d20"                 # roll 2 20 sided dice and sum the result
rq "[1, 10)"              # random decimal between 1 and 10, not including 10
rq "a, b, c / shuffle"    # shuffle the list
rq "coin, d6, -5..5 / 2"  # 2 values from the list and evaluate the result
cat file.txt | rq "/10o"  # choose 10 lines keeping original order
```

## CLI

```sh
# Install it
cargo install rng-query

# Basic usage
rq "your query"   # run a quick query
rq --help         # see help message
```

There are also precompiled binaries in the github releases.

With the CLI you can read entries from `stdin`. Each line will be an entry, but
just as text. You can pass the `-e` flag to evaluate each line as a separate
expression. Then the query you execute will have the entries of stdin already
included.

Input files will be stored in memory with a little overhead. Therefore, very
large files may use a lot of memory. It is possible to improve this, but it's
currently not in the scope of this project.

## Syntax

A query is a selection of entries from a list with options. Each entry can be
an expression or just text.

Entries are separated by a comma `,` or a new line. Then, everything after `/`
until the end of the query will be options.

### Options

If the input ends without options, `/ 1` is the default.

The options have the format `/ [n] [flags]`, where flags are just chars. Spaces
are ignored. `[n]` is a non negative integer or `all`. If not given, it's 1.

Flags are single characters, they can be separated with spaces and cannot
repeat. The flags are:

- `r`: allow options to repeat.
- `o`: keep the original order when choosing multiple.

There are some presets with better names for the operation:

- `/ shuffle` same as `/ all`
- `/ list` same as `/ all o`

### Expressions

Each entry can be an expression, there are currently 4 expressions:

- [Subqueries](#subqueries)
- [Intervals](#intervals)
- [Dice](#dice)
- [Coin](#coin)

#### Subqueries

Another query can be nested between `{` and `}`. When selected,
will be evaluated the same as the root one. This allows to express a bit more
of a bias towards some options.

For example:

```sh
"a, {b, c}"  # a query where 'a' has a 50% chance of being selected and 'b'
              # and 'c' split the other 50%, so 25% each
```

#### Intervals

Choose a random number. Between `[` or `(` and `]` or `)`. The
bounds are determined by the enclosing character. Square brackets include the
bound and parenthesis dont.

Inside 2 numbers represent the lower and upper limit. If they are separated
by a comma `,` the will be treated as floats. If they are separated with `..`
it will be floats if any of them are. Some examples are:

```sh
"(1, 5)"   # decimal between 1 and 5, not included
"(1, 5]"   # decimal between 1 and 5, 5 included
"[1..5]"   # integer between 1 and 5
"[1..5.5)" # decimal betwen 1 and 5.5, not included
```

An alternative syntax just for integers is a range like this:

```sh
"1..5"  # integer between 1 and 4
"1..=5" # integer between 1 and 5
```

Negatives number are supported both in integers and floats.

Open/half-open intervals are not supported because I don't know a good way to
handle max/min values.

#### Dice

Roll dice with a D&D like syntax.

```txt
[amount]d<sides>[!][select][modifier*]

d6        => roll a 6 sided die
2d6       => 2 x 6s dice and sum
2d20k     => 2 x 20s dice and keep the highest
```

Sides can also be `%` which equals to `100`.

`!` is exploding. If rolled the maximum value, roll another die.

For select you can add `<k|d>[h|l][n]`. If `n` is not given, it's 1. You can
have:

- `k` or `kh` to keep the highest `n` dice.
- `kl` to keep the lowest `n` dice.
- `d` or `dl` to drop the lowest `n` dice.
- `dh` to drop the `n` highest dice.

The modifer is `<+|->[m]` to add or subtract a value to the total result. You
can specify more than one.

When evaluated you will get the sum of all the dice rolls.

There are many more ways to expand this dice notation, but please don't use
this tool for your D&D game, roll real dice! If you really *really* **really**
think more modifiers can be useful, submit an issue.

#### Coin

Toss a coin. Simple, just write `coin`. At the end it's equivalent to a subquery
like `{ heads, tails }`.

## Notes on pseudorandomness

Currently, randomness should be statistically valid, but NOT cryptographically
secure. If you need this to change, submit an issue.
