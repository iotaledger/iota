import { Module } from '@nestjs/common';
import { MonitorNetworkController } from './monitor-network.controller';

@Module({
  controllers: [MonitorNetworkController],
})
export class MonitorNetworkModule {}
