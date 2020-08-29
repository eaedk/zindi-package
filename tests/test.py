from zindi.user import Zindian


user = Zindian("")

user.which_challenge

user.select_a_challenge()

user.which_challenge

user.leaderboard()

user.my_rank

user.download_dataset(destination=".")

# user.submit(filepaths=['sample_data/california_housing_test.csv'])

user.submission_board()

user.create_team(team_name="New Team")