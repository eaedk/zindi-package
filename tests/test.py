import sys, os

## To avoid errors of importing before instalation
currentdir = os.path.dirname(os.path.realpath(__file__))
parentdir = os.path.dirname(currentdir)
sys.path.append(parentdir)

from zindi.user import Zindian


user = Zindian("")

user.which_challenge

user.select_a_challenge(fixed_index=None)

# user.which_challenge

# user.leaderboard()

# user.my_rank

# user.download_dataset(destination="tests/dataset")

user.submit(filepaths=['tests/dataset/SampleSubmission.csv'])

user.submission_board()

user.create_team(team_name="New Team")