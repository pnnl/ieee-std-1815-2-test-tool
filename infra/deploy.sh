#  Prerequisites:
#  - Azure CLI installed and logged in
#  - Subnet created
#      - Subnet delegated to Microsoft.ContainerInstance/containerGroups
set -euxo pipefail

source ./infra/.env

GIT_COMMIT="$(git rev-parse HEAD)"
IMAGE_TAG="${GIT_COMMIT:0:7}"

function setup_azure() {
    az cloud set --name AzureCloud
    az account set --subscription "IEEE 1815.2 Reference Stations"
}

function deploy_acr_and_outstation() {
    acr_results="$(az deployment group create \
    --resource-group $RESOURCE_GROUP \
    --name outstation-acr-deployment \
    --template-file ./infra/1_one_time_setup.bicep \
    --parameters acrName=$ACR_NAME \
    --query properties.outputs \
    --output json)"

    ACR_LOGIN_SERVER="$(jq -r '.acrLoginServer.value' <<< "$acr_results")"
    PULL_IDENTITY_ID="$(jq -r '.pullIdentityId.value' <<< "$acr_results")"

    az acr build \
    --registry $ACR_NAME \
    --image $IMAGE_TAG \
    --file ./infra/Dockerfile \
    .

    az deployment group create \
    --resource-group $RESOURCE_GROUP \
    --name outstation-container-deployment \
    --template-file ./infra/2_main.bicep \
    --parameters containerImage="${ACR_LOGIN_SERVER}/${IMAGE_TAG}" \
                acrLoginServer=$ACR_LOGIN_SERVER \
                pullIdentityId=$PULL_IDENTITY_ID \
                subnetId=$SUBNET_ID
}

function main () {
    setup_azure
    deploy_acr_and_outstation
}

main
