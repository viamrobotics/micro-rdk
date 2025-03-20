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


async def connect(api_key: str, api_key_id: str, num_attempts: int) -> ViamClient:
    for i in range(num_attempts):
        try:
            dial_options = DialOptions.with_api_key(api_key_id=api_key_id, api_key=api_key)
            viam_client = await ViamClient.create_from_dial_options(dial_options)
            return viam_client
        except Exception as e:
            if i == num_attempts-1:
                raise e
            print(e)
        time.sleep(0.5)

async def main():
    api_key = os.environ["ESP32_CANARY_API_KEY"]
    api_key_id = os.environ["ESP32_CANARY_API_KEY_ID"]
    part_id = os.environ["ESP32_CANARY_ROBOT_PART_ID"]
    tag_name = os.environ["ESP32_CANARY_OTA_VERSION_TAG"]
    bin_name = os.environ["ESP32_OTA_BINARY_NAME"]

    url = f"https://github.com/viamrobotics/micro-rdk/releases/download/{tag-name}/{bin_name}"

    print(f"connecting ViamClient ...")

    viam_client = await connect(api_key, api_key_id, 5)

    cloud = viam_client.app_client

    print(f"getting robot part...")
    robot_part = await cloud.get_robot_part(robot_part_id=part_id)

    service_updated = False
    updated_config = copy.copy(robot_part.robot_config)
    for service in updated_config["services"]:
        # assumes only one such service exists
        if service["model"] == "ota_service":
            service["attributes"]["url"] = url
            service["attributes"]["version"] = tag_name
            service_updated = True
            print(f"updating OtaServiceConfig version to `{tag_name}`")
            break

    if not service_updated:
        viam_client.close()
        msg = f"failed to find or update ota service config to `{tag_name}`"
        post_to_slack(msg, True)
        raise Exception(msg)

    print("updating robot part config...")
    await cloud.update_robot_part(
        robot_part_id=robot_part.id,
        name=robot_part.name,
        robot_config=updated_config
    )

    print("verifying config change...")
    robot_part = await cloud.get_robot_part(robot_part_id=part_id)
    for service in robot_part.robot_config["services"]:
        if service["model"] == "ota_service":
            print(f"OtaServiceConfig after updating: `{service}`")
            if service["attributes"]["url"] != url or service["attributes"]["version"] != tag_name:
                msg = f"ota service config does not reflect update to `{tag_name}`"
                post_to_slack(msg, True)
                raise Exception(msg)

    viam_client.close()
    post_to_slack(f"OtaService config successfully updated to `{tag_name}`", False)

def post_to_slack(msg: str, is_error: bool):
    today = datetime.datetime.now(tz=tz.UTC).date()
    msg = f"{today}: {msg}"
    slack_token = os.environ["CANARY_SLACKBOT_TOKEN"]
    channel_id = os.environ["MICRO_RDK_TEAM_CHANNEL_ID"]
    client = slack_sdk.WebClient(token=slack_token)
    api_result = client.chat_postMessage(channel=channel_id, text=msg)

    try:
        api_result.validate()
        if is_error:
            raise Exception(msg)
    except Exception as e:
        raise Exception(f"failure to post to Slack, error message was '{msg}'") from e


if __name__ == '__main__':
    asyncio.run(main())
