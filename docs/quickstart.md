# Your first submission

This walk-through takes a competition from discovery to the leaderboard. The command line
and Python versions do exactly the same thing; pick the one you like.

## 1. Log in

```bash
zindi login
```

You type your username and password once. Only the returned token is saved (see
[Authentication](authentication.md)); your password never touches the disk.

## 2. Find a competition

=== "CLI"

    ```bash
    zindi competitions                    # open ones
    zindi competitions --search soil      # search titles
    zindi competitions --reward prize     # prize, points or knowledge
    zindi competitions --all              # include closed ones (great for practice)
    ```

=== "Python"

    ```python
    import zindi
    client = zindi.Client()
    client.competitions(search="soil", reward="prize")
    ```

## 3. Read it, then join

```bash
zindi show Crop-Yield-Forecast-Challenge --rules
zindi use Crop-Yield-Forecast-Challenge    # remember it in ./zindi.toml
zindi join
```

`zindi use` writes a small `zindi.toml` in your project folder. From then on, commands run
in that folder (or any sub-folder) do not need the competition id. Private competitions
ask for their secret code: `zindi join --code XXXX`.

## 4. Get the data

```bash
zindi download               # into ./data, skips files you already have
zindi download -o input -f Train.csv -f Test.csv --unzip
```

## 5. Submit

```bash
zindi submit submission.csv -m "lightgbm, 5 folds" --wait
```

With `--wait`, the command waits until Zindi has scored the file and prints the score.
It also tells you how many submissions you have left today.

=== "Python"

    ```python
    sub = client.submit("submission.csv", comment="lightgbm, 5 folds", wait=True)
    sub.public_score
    client.limits().remaining_today
    ```

## 6. See where you stand

```bash
zindi leaderboard -n 20
zindi rank
zindi submissions
```

## 7. Team up (optional)

```bash
zindi team create "Lagos Learners" -i friend_username
zindi team show
```
