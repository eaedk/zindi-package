def get_challenges(reward='', kind='', active='', url='', headers='' ):
    """Get the available Zindi's challenges using filter options.

    Parameters
    ----------
    url : string
        The url of the selected challenge.
    headers : dictionnary
        The headers of the request to participate in a challenge.    
    Returns
    -------
    challenges_data : pd.DataFrame
        The initial raw predictions. K is equal to 1 for binary
        classification and regression, and equal to the number of classes
        for multiclass classification.
    """