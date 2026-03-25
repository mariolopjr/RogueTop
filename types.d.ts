interface PendingMigration {
  action: 'copy' | 'move'
  from: string
  to: string
}

interface Config {
  skip_splash: boolean
  offline: boolean
  rpc: boolean
  name: string
  pending_migration: PendingMigration | null
}