# zindi-package

## Description
A user-friendly ZINDI package which allow Zindians to achieve all available tasks on ZINDI Platform using this package.

## Usage
```python
# create a user object
from zindi.user import Zindian

my_username = "I_am_Zeus_AI"
user = Zindian(username = my_username)


# get current challenge info if seleted
user.which_challenge

# select a new challenge
user.select_a_challenge()

# user.which_challenge

# user.leaderboard()

# user.my_rank

# user.download_dataset(destination="tests/dataset")

user.submit(filepaths=['tests/dataset/SampleSubmission.csv'])

user.submission_board()

user.create_team(team_name="New Team")

``` 

## Status
In progress 🚧 . Check the progression state in the ` features.txt ` file.
