class Url:
    signIn = "https://api.zindi.africa/v1/auth/signin"
    base_api = "https://api.zindi.africa/v1/competitions"

    def __init__(self, challenge_id):
        
        #api link on which all requests will be based
        self.api = f"{self.base_api}/{challenge_id}"

        self._ = f"{self.api}/"
        self._ = f"{self.api}/"
        self._ = f"{self.api}/"
        self._ = f"{self.api}/"
        self._ = f"{self.api}/"
        self._ = f"{self.api}/"

