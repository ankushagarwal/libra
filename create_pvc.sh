#!/bin/bash

cat << EOF  | kubectl apply -f-
allowVolumeExpansion: true
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: teststorageclass
parameters:
  iopsPerGB: "50"
  type: io1
provisioner: kubernetes.io/aws-ebs
reclaimPolicy: Delete
volumeBindingMode: WaitForFirstConsumer
EOF


for ((i = 0; i < 30; i++)); do
cat << EOF  | kubectl apply -f-
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: validator-${i}
  labels:
    libra-node: "true"
spec:
  storageClassName: teststorageclass
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 50Gi
EOF
done

for ((i = 0; i < 30; i++)); do
cat << EOF  | kubectl apply -f-
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: fullnode-${i}-0
  labels:
    libra-node: "true"
spec:
  storageClassName: teststorageclass
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 50Gi
EOF
done
