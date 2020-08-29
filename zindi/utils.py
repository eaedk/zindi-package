import requests
from tqdm import tqdm
import pandas as pd

# Utils

## Download a file

def download(url="https://", filename="", headers=''):

    response = requests.post(url, headers=headers, stream=True)
    total = int(response.headers.get('content-length', 0))
    with open(filename, 'wb') as file, tqdm(
            desc=filename,
            total=total,
            unit='o',
            unit_scale=True,
            unit_divisor=1024,
    ) as bar:
        for data in response.iter_content(chunk_size=1024):
            size = file.write(data)
            bar.update(size)


## Print
### Challenges
def print_challenges(challenges_data):

    print("_"*130 )
    print('|{:^5}|{:^14}|{:^18.18}|{:^16.16}|{:^10}'.format( "" , "" , "", "" , "" ,  ))
    print('|{:^5}|{:^14}|{:^18.18}|{:^16.16}|{:^10}'.format( "index" , "challenge" , "problem", "reward" , "id" ,  ))
    print('|{:^5}|{:^14}|{:^18.18}|{:^16.16}|{:^10}'.format( "" , "" , "", "" , "" ,  ))

    n_challenges = challenges_data.shape[0]

    for i in range(n_challenges):
        data = dict(challenges_data.iloc[i])
        reward = data['reward']
        id = data['id']
        problem = '' if  len(data['type_of_problem'])==0 else data['type_of_problem'][0]
        kind = "Hack" if data['kind'] == "hackathon" else "Compet"
        visibility = 'Public' if not data['secret_code_required'] else 'Private'
        challenge = f"{visibility} {kind}"
        
        print("-"*130 )
        print ('|{:^5}|{:^14}|{:^18.18}|{:^16.16}|{:^10}'.format( i , challenge , problem , reward , id ,  ))
    print(f'{"_"*130}\n\n' )   

## Print
### Leaderboard
def print_lb(challengers_data, my_rank):

    print("_"*130 )
    print('|{:^6}|{:^20}|{:^44}|{:^12}|{:^12}'.format( "" , "" , "", "" ,"" ,  ))
    print('|{:^6}|{:^20}|{:^44}|{:^12}|{:^12}'.format( "rank" , "score" , "name", "counter", "last_submission" ,  ))
    print('|{:^6}|{:^20}|{:^44}|{:^12}|{:^12}'.format( "" , "" , "", "" ,"" ,  ))


    # for data in challengers_data:
    for i in range(len(challengers_data)):
        data = challengers_data[i]
        try:
            score = data['best_private_score'] if "best_private_score" in data else data['best_public_score']
            rank = data['private_rank'] if "private_rank" in data else data['public_rank']
            name = data['user']['username'] if "user" in data else f"TEAM - {data['team']['title']}"
            n_submission = data['submission_count']
            last_submission = data['best_private_submitted_at'] if "best_private_submitted_at" in data else data['best_public_submitted_at']
            
            check = str(last_submission)==str(None) # conditioin to set last_submission value
            last_submission = '' if check==True else pd.to_datetime(str(last_submission)).strftime("%d %B %Y, %H:%M")

            if rank != None: 
                if rank == my_rank:
                    name += "🟢"
                print("-"*130 )
                print('|{:^6}|{:^20}|{:^44.44}|{:^12}|{:^12}'.format( str(rank) , str(score) , str(name) , str(n_submission), str(last_submission) ,  ))
            else:
                break # all the next zindians haven't made a correct submission yet
        except Exception as e:
            print(e)
    print(f'{"_"*130}\n\n' )  

## Print
### Submission-board
def print_submission_board(submissions_data):

    print("_"*130 )
    print('|{:^6}|{:^10}|{:^18}|{:^16}|{:^30} |{:^25}'.format( "" , "" , "", "" ,"" , "" ,  ))
    print('|{:^6}|{:^10}|{:^18}|{:^16}|{:^30} |{:^25}'.format( "status" , "id" , "date", "score", "filename", "comment" ,  ))
    print('|{:^6}|{:^10}|{:^18}|{:^16}|{:^30} |{:^25}'.format( "" , "" , "", "" ,"" , "" ,  ))

    for data in submissions_data:
        filename = data['filename']
        date = pd.to_datetime(data['created_at'] ).strftime("%d %b %Y, %H:%M")
        id = data['id']
        
        if data["status"] == "successful": 
            status = "🟢"
            score = data['private_score'] if "private_score" in data else data['public_score']
            comment = data['comment'] 
        else:
            score = "-"
            status = "🔴"
            comment = data['status_description']
        print("-"*130 )
        print('|{:^5}|{:^10}|{:^12}| {:^14.14} |{:30.30} |{:40.40}'.format( status , id , date, str(score) , filename, comment ,  ))
    print(f'{"_"*130}\n\n' )  


## Join challenge
def join_challenge( url, headers, ):

    response = requests.post( url, headers=headers )
    response = response.json()['data']
    try:
        print(f"\n[ 🔴 ] {response['errors']}\n")
    except:
        print(f"\n[ 🟢 ] {response}\n")


## Show available challenges
def get_challenges(reward='all', kind='all', active='all', url='', headers='' ):

    to_show_challenge_data = ["id","kind",  "subtitle", "reward", "type_of_problem", "data_type", "secret_code_required", "sealed"]
    challenges_data = pd.DataFrame()
    
    # check validity of challenge sorting's values
    reward = '' if reward.lower() not in ['prize', 'points', 'knowledge' ] else reward.lower()
    kind = '' if kind.lower() not in ['competition', 'hackathon'] else kind.lower()
    active = '' if active.lower() not in [True, False] else int(active)
    # join sorting params in a dictionary which will be passed in the url
    sorting_params = dict(page=0, per_page=200, reward=reward, kind=kind, active=active )
    
    # request
    response = requests.get( url, headers=headers , params=sorting_params)
    response = response.json()["data"]
    try: 
        print( response["errors"] )
    except:
        challenges_data = pd.DataFrame(response)
        # extract relevant columns and print the recap table
        challenges_data = challenges_data[to_show_challenge_data]
        print_challenges( challenges_data )
    return challenges_data


##  Info about the challenges user participate in
def participations(challenge_id, headers):
    url = "https://api.zindi.africa/v1/participations"
    response = requests.get( url, headers=headers )
    response = response.json()["data"]
    team_id = response[challenge_id]['team_id']
    return team_id


## Info about user position on lb 
def me_on_leaderboard(challengers_data, challenge_id, username, headers):
    df_lb = pd.DataFrame(challengers_data)
    my_rank_on_lb = -1
    df_lb = df_lb[ (df_lb.public_rank != None) ] # filter to use only on leaderboard challengers
    team_id = participations(challenge_id=challenge_id, headers=headers)
    # Simple user
    if  not team_id:
        user = df_lb[df_lb['user'].astype(str).str.contains( username,)] # na=False, case=True, regex=False
        my_index_on_lb = user.index.values[0].astype(int)
    # Team
    else :
        my_team = df_lb[df_lb['team'].astype(str).str.contains(team_id,)]
        my_index_on_lb = my_team.index.values[0].astype(int)

    my_rank_on_lb = my_index_on_lb + 1

    return my_rank_on_lb