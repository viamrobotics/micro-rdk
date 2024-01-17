import asyncio
import datetime
import os

from dateutil import tz
from pymongo import MongoClient

FAILURE_ACCEPTABILITY = 0.2

def main():
    today = datetime.datetime.now(tz=tz.UTC).date()
    tomorrow = today + datetime.timedelta(days=1)
    start_of_day = datetime.datetime(today.year, today.month, today.day, tzinfo=tz.UTC)
    start_of_tomorrow = datetime.datetime(tomorrow.year, tomorrow.month, tomorrow.day, tzinfo=tz.UTC)

    mongo_connection_str = os.environ["MONGODB_TEST_OUTPUT_URI"]
    db_client = MongoClient(mongo_connection_str)
    db = db_client["micrordk_canary"]
    coll = db["raw_results"]

    print("getting hourly results...")

    result_set = coll.find({ "_id": { "$gte": start_of_day, "$lt": start_of_tomorrow } })
    latency_sum = 0
    successes = 0
    connection_failures = 0
    board_api_successes = 0
    board_api_failures = 0
    connection_attempts = 0
    num_results = 0
    for record in result_set:
        num_results += 1
        if record["connection_success"]:
            latency_sum += record["connection_latency_ms"]
            connection_attempts += record["connection_attempts"]
            successes += 1
            board_api_successes += record["board_api_successes"]
            board_api_failures += record["board_api_failures"]
        else:
            connection_failures += 1
    
    if num_results == 0:
        raise Exception(f"no hourly canary results found for {today}")

    avg_connection_latency_ms = latency_sum / successes if successes != 0 else 0
    avg_connection_attempts = connection_attempts / num_results
    
    total_board_calls = board_api_successes + board_api_failures

    coll2 = db["daily_summaries"]
    summary = {
        "_id": start_of_day,
        "successes": successes,
        "robot_connection_failures": connection_failures,
        "board_api_failures": board_api_failures,
        "avg_connection_latency_ms": avg_connection_latency_ms,
        "avg_connection_attempts": avg_connection_attempts
    }
    inserted_id = coll2.insert_one(summary)
    print(f"successfully inserted stats for {inserted_id}: {summary}")

    failure_rate = round(connection_failures / num_results, 3)
    if failure_rate > FAILURE_ACCEPTABILITY:
        raise Exception(f"Connection failure rate {failure_rate * 100}%% greater than {FAILURE_ACCEPTABILITY * 100}%%")
    
    board_failure_rate = round(board_api_failures / total_board_calls)
    if board_failure_rate > FAILURE_ACCEPTABILITY:
        raise Exception(f"Connection failure rate {failure_rate * 100}%% greater than {FAILURE_ACCEPTABILITY * 100}%%")

if __name__ == '__main__':
    main()
