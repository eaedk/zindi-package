# Imports

import sys, os

## To avoid errors of importing before instalation
currentdir = os.path.dirname(os.path.realpath(__file__))
parentdir = os.path.dirname(currentdir)
sys.path.append(parentdir)

from zindi.utils import *
from getpass import getpass

import pandas as pd
import requests
from tqdm import tqdm


# Class declaration and init
class Zindian:
    # __auth_data = ""
    
    def __init__(self, username):
        self.__headers = {'User-Agent' : "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/83.0.4103.116 Safari/537.36"}
        self.__signin(username)
        self.__challenge_selected = False

# Properties
    @property
    def which_challenge(self,):
        if self.__challenge_selected:
            msg = f"[ 🟢 ] You are currently enrolled in : {self.__challenge_data['id']} challenge,\n{self.__challenge_data['subtitle']}." 
            challenge = self.__challenge_data['id']

        else:
            msg = f"[ 🔴 ] You have not yet selected any challenge."
            challenge = None
        print(msg)
        return challenge

    @property
    def my_rank(self,):
        if self.__challenge_selected:
            self.leaderboard(to_print=False)
            int_rank = self.__rank
            if int_rank == 0:
                rank = f"not yet"
            elif str(int_rank)[-1] == "1":
                rank = f"{int_rank}st"
            elif str(int_rank)[-1] == "2":
                rank = f"{int_rank}nd"
            elif str(int_rank)[-1] == "3":
                rank = f"{int_rank}rd"
            else:
                rank = f"{int_rank}th"
            msg = f"[ 🟢 ] You are {rank} on the leaderboad of {self.__challenge_data['id']} challenge, Go on..." 
        else:
            msg = f"[ 🔴 ] You have not yet selected any challenge."
        print(msg)
        return int_rank

# Account
## Sign In
    def __signin(self, username ):
        
        url = "https://api.zindi.africa/v1/auth/signin"
        password = getpass(prompt='Your password\n>> ')
        data = {"username": username, "password": password }

        ## FOR TEST
        #USERNAME, PASSWORD = "eaedk", getpass("password : ")
        #data = {"username": USERNAME, "password": PASSWORD }

        response = requests.post( url, data=data , headers=self.__headers )
        response = response.json()["data"]
        try: 
            print( f'[ 🔴 ] {response["errors"]}' )
        except:
            print( f"[ 🟢 ] 👋🏾👋🏾 Welcome {response['user']['username'] } 👋🏾👋🏾" )
            self.__auth_data = response
            
# Challenge
## Select a challenge to participate in
    def select_a_challenge(self, reward='all', kind='all', active='all',):
        
        headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
        url = "https://api.zindi.africa/v1/competitions" 
        challenges_data = get_challenges(reward=reward, kind=kind, active=active, url=url, headers=headers )
        print_challenges( challenges_data=challenges_data )
        while True:
            # challenge_index = input("Type index of the challenge you want to participate in.\n>>")
            try:
                # challenge_index = int( challenge_index.strip() )
                challenge_index = 3
                self.__challenge_data = challenges_data.iloc[challenge_index]
                self.__api = f"https://api.zindi.africa/v1/competitions/{self.__challenge_data['id']}"
                self.__challenge_selected = True

                print(f"\n\n[ 🟢 ] You choose challenge : {self.__challenge_data['id']},\n{self.__challenge_data['subtitle']} .\n")
                # Stop the inifinite loop when a challenge is choose
                break
            except:
                print("\n\n[ 🔴 ] Please enter a correct challenge index.\n")

## Download dataset
    def download_dataset(self, destination="." ):
        if os.path.isdir(destination):
            if self.__challenge_selected :
                headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
                url = self.__api

                response = requests.get( url,  headers=headers )
                datafiles = response.json()['data']['datafiles']

                # DOWNLOAD FILES USING ANOTHER METHOD TO SAVE THE ABILITY OF MULTIPROCESSING
                [ download(url=f"{url}/files/{data['filename']}", filename=os.path.join(destination, data['filename']), headers=headers) 
                for data in datafiles]
                
            else:
                print(f"\n[ 🔴 ] You have to select a challenge before to downoad a dataset,\nuse the select_a_challenge method before.\n")
        else:
            print(f"\n[ 🔴 ] You have to choose a correct destination's path for the dataset.\n")

## Push submission file
    def submit(self, filepaths=[], comments=[]):

        if self.__challenge_selected :
            headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
            url = f"{self.__api}/submissions"
            allowed_extensions = [ 'csv', ]
            if len(comments) < len(filepaths):
                n_blank_comment = len(filepaths) - len(comments)
                comments += [""]*n_blank_comment

            for filepath, comment in zip(filepaths, comments):
                extension = filepath.split(".")[-1].strip().lower()
                if extension in allowed_extensions:
                    if os.path.isfile(filepath):
                        file = {"file": open(filepath, 'rb')} # Loading the file to be submitted
                        data = {"comment": comment}
                        response = requests.post(url, headers=headers, files=file, data=data,)
                        response = response.json()['data']
                        try:
                            print(f"\n[ 🔴 ] Something wrong with file :{filepath} ,\n{response['errors']}\n")
                        except:
                            print(f"\n[ 🟢 ] Submission ID: {response['id'] } - File submitted : {filepath}\n")
                    else:
                        print(f"\n[ 🔴 ] File doesn't exists, please verify this filepath : {filepath}\n")
                else:
                    print(f"\n[ 🔴 ] Submission file must be a CSV file ( .csv ), please verify this filepath : {filepath}\n")
        else:
            print(f"\n[ 🔴 ] You have to select a challenge before to push any submission file,\nuse the select_a_challenge method.\n")

## Show leaderboard
    def leaderboard(self, to_print=True):

        if self.__challenge_selected :
            headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
            per_page = 100000
            url = f"{self.__api}/participations"
            params_in_url = { "page": 0, "per_page": per_page,} #  "ended": True 

            response = requests.get( url, headers=headers, params=params_in_url )
            # per_page = response.json()['meta']['total_count']
            response = response.json()['data']
            try:
                print(f"\n[ 🔴 ] {response['errors']}\n")
            except:
                challengers_data = response
                self.challengers_data = response
                self.__rank = user_on_lb(challengers_data, challenge_id=self.__challenge_data['id'], username=self.__auth_data['user']['username'], headers=headers)
                if to_print:
                    print_lb(challengers_data=challengers_data, user_rank=self.__rank)
        else:
            print(f"\n[ 🔴 ] You have to select a challenge before to get the leaderboard,\nuse the select_a_challenge method.\n")
    
## Show Submission-board

    def submission_board(self, ):
        #to add : number of submission, available subissions to do
        if self.__challenge_selected :
            url = f"{self.__api}/submissions"
            headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }

            params_in_url = { "per_page": 1000 } # per_page : max number of subimission to retrieve
            response = requests.get( url, headers=headers, params=params_in_url )
            response = response.json()['data']
            try:
                print(f"\n[ 🔴 ] {response['errors']}\n")
            except:
                print_submission_board(submissions_data=response)
        else:
            print(f"\n[ 🔴 ] You have to select a challenge before to get the submission-board,\nuse the select_a_challenge method.\n")


# Team
## Create
    def create_team(self, team_name=""):

        if self.__challenge_selected :
            headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
            url = f"{self.__api}/my_team"
            data = {"title": team_name}

            response = requests.post( url, headers=headers, data=data )
            response = response.json()['data']
            try:
                print(f"\n[ 🔴 ] {response['errors']}\n")
            except:
                print(f"\n[ 🟢 ] {response}\n")
        else:
            print(f"\n[ 🔴 ] You have to select a challenge before to manage your team,\nuse the select_a_challenge method.\n")

## Team Up
    def team_up(self, zindians=[]):

        if self.__challenge_selected :
            headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
            url = f"{self.__api}/my_team/invite"

            for zindian in zindians:
                data = {"username": zindian}
                response = requests.post( url, headers=headers, data=data )
                response = response.json()['data']
                try:
                    print(f"\n[ 🔴 ] {response['errors']}\n")
                except:
                    print(f"\n[ 🟢 ] {response}\n")
        else:
            print(f"\n[ 🔴 ] You have to select a challenge before to manage your team,\nuse the select_a_challenge method.\n")

## Disband
    def disband_team(self, ):

        if self.__challenge_selected :
            headers = {**self.__headers, "auth_token":self.__auth_data["auth_token"] }
            url = f"{self.__api}/my_team"

            response = requests.delete( url, headers=headers,)
            response = response.json()['data']
            try:
                print(f"\n[ 🔴 ] {response['errors']}\n")
            except:
                print(f"\n[ 🟢 ] {response}\n")
        else:
            print(f"\n[ 🔴 ] You have to select a challenge before to manage your team,\nuse the select_a_challenge method.\n")
