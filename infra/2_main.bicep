targetScope = 'resourceGroup'

param containerImage string
param acrLoginServer string
param pullIdentityId string
param subnetId string

var namePrefix = 'reference-outstation'
var containerGroupName = '${namePrefix}-container-group'
var dnp3Port = 20000

resource cg 'Microsoft.ContainerInstance/containerGroups@2023-05-01' = {
  name : containerGroupName
  location : resourceGroup().location
  identity: {
    type: 'UserAssigned'
    userAssignedIdentities: {
      '${pullIdentityId}': {}
    }
  }
  properties: {
    osType: 'Linux'
    restartPolicy: 'Always'
    subnetIds: [
      {
        id: subnetId
      }
    ]
    imageRegistryCredentials: [
      {
        server: acrLoginServer
        identity: pullIdentityId
      }
    ]
    ipAddress: {
      type: 'Private'
      ports: [
        {
          protocol: 'TCP'
          port: dnp3Port
        }
      ]
    }
    containers: [
      {
        name: 'reference-outstation'
        properties: {
          image: containerImage
          resources: {
            requests: {
              cpu: 1
              memoryInGB: 1
            }
          }
          ports: [
            {
              protocol: 'TCP'
              port: dnp3Port
            }
          ]
        }
      }
    ]
  }
}

output privateIp string = cg.properties.ipAddress.ip
