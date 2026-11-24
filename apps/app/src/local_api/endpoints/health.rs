use contracts::Empty;

contracts::endpoints! {
    group health;

    CheckHealth: Get "/health", None, (), (), Empty => Empty;
}
