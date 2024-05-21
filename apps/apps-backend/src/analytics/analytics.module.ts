import { Module } from '@nestjs/common';
import { AnalyticsController } from './analytics.controller';

@Module({
  controllers: [AnalyticsController],
})
export class AnalyticsModule {}
