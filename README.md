# tasky-api
[![codecov](https://codecov.io/gh/tasky-aws/tasky-api/branch/master/graph/badge.svg)](https://codecov.io/gh/AwesomeIbex/grekko)
![Audit-Check](https://github.com/tasky-aws/tasky-api/workflows/Audit-Check/badge.svg)

_This was going through a period of rapid development to get a proof of concept working. I'm now slowly going through and adding
better features/refactoring, so bear with me on the test/abstraction scenario._

This is the api for Tasky. It handles all requests to AWS, at present it handles retrieving logs from cloud watch in a stream-like 
fashion, retrieving all ECS data.


Once the ZMQ integration is finished, i'll be happy to spend a bit more work getting this ready and refactoring. The intention is to support more
stream-like AWS content and lets see if anything else with their UI will make me unhappy.
## Getting started

### Dependencies
There are none, its an executable

Theres some work to do for me to be happy with this as a proof of concept:
- Write a decent readme
- Write tests and post coverage (i have code for this but theres some work to do)

Here's a dump of my TODO list:
```
Tasky GUI
- Query api for regions - maybe
- Style cloudwatch logs
- reset button

Done:
- Role management and state
- Ecs load clusters, services and tasks in a tree like fashion
- Chips at the top for ecs stats
- Ecs > cloudwatch logs 
- Expose parameters in logs like days, amount to query and a filter
- SSE for alerts
- Notifications for task stoppages by comparing with store every x seconds, delegate to service
- Could query api for roles or use a role manager to add to state. Could read from credentials file
- ECS > parameter store for arns 
- Notification > task logs
- If errors come through, logs on fire

Improvements:
- Loop through a list of roles and get them alll
- Change notifications from SSE to Zmq


```

# DISCLAIMER
If tasky blows up your computer, loses your job or just deletes all your everything, I am NOT liable for anything. Also see the liability clause at the end of the new BSD licence text in the COPYRIGHT file.
