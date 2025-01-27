import asyncio
import time
import datetime
import copy
import os

import slack_sdk
import slack_sdk.errors
from dateutil import tz

from viam.robot.client import DialOptions
from viam.app.viam_client import ViamClient

async def connect(robot_address: str, api_key: str, api_key_id: str) -> ViamClient:
    dial_options = DialOptions.with_api_key(api_key_id=api_key_id,api_key=api_key)
    return await ViamClient.create_from_dial_options(dial_options)

async def try_connect(robot_address: str, api_key: str, api_key_id: str) -> ViamClient:
    for i in range(5):
        try:
            viam_client = await connect(robot_address, api_key, api_key_id)
            return viam_client
        except Exception as e:
            if i == 4:
                raise e
            print(e)
        time.sleep(0.5)

async def main():

    robot_address = os.environ["ESP32_CANARY_ROBOT"]
    api_key = os.environ["ESP32_CANARY_API_KEY"]
    api_key_id = os.environ["ESP32_CANARY_API_KEY_ID"]
    part_id = os.environ["ESP32_CANARY_ROBOT_PART_ID"]
    tag_name = os.environ["ESP32_CANARY_OTA_VERSION_TAG"]
    bucket_url = os.environ["GCP_BUCKET_URL"]
    bucket_name = os.environ["GCP_BUCKET_NAME"]
    bin_name = "micro-rdk-server-esp32-ota.bin"
    
    url = f"{bucket_url}/{bucket_name}/{tag_name}/{bin_name}"
        
    print(f"connecting to robot at {robot_address} ...")
    
    viam_client = await try_connect(robot_address, api_key, api_key_id)

    cloud = viam_client.app_client

    robot_part = await cloud.get_robot_part(robot_part_id=part_id)

    service_updated = False
    updated_config = copy.copy(robot_part.robot_config)
    for service in updated_config["services"]:
        # assumes only one such service exists
        if service["model"] == "ota_service":
            service["attributes"]["url"] = url
            service["attributes"]["version"] = tag_name            
            service_updated = True
            print(f"updating OtaServiceConfig to `{service}`")
            break

    if not service_updated:
        viam_client.close()
        raise Exception("failed to find or update ota service config")

    await cloud.update_robot_part(
        robot_part_id=robot_part.id,
        name=robot_part.name,
        robot_config=updated_config
    )

    # retrieve new config to verify
    robot_part = await cloud.get_robot_part(robot_part_id=part_id)
    for service in robot_part.robot_config["services"]:
        if service["model"] == "ota_service":
            print(f"OtaServiceConfig after updating: `{service}`")
            if service["attributes"]["url"] != url or service["attributes"]["version"] != tag_name:
                raise Exception("ota service config does not reflect update")

    viam_client.close()

if __name__ == '__main__':
    asyncio.run(main())
    
def post_to_slack(msg: str):
    today = datetime.datetime.now(tz=tz.UTC).date()
    slack_token = os.environ["CANARY_SLACKBOT_TOKEN"]
    channel_id = os.environ["MICRO_RDK_TEAM_CHANNEL_ID"]
    client = slack_sdk.WebClient(token=slack_token)
    api_result = client.chat_postMessage(channel=channel_id, text=msg)

    try:
        api_result.validate()
        raise Exception(msg)
    except Exception as e:
        raise Exception(f"failure to post to Slack, error message was '{msg}'") from e 
