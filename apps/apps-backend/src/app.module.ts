import { Module } from '@nestjs/common';
import { PricesModule } from './prices/prices.module';
import { FeaturesModule } from './features/features.module';
import { MonitorNetworkModule } from './monitor-network/monitor-network.module';
import { AnalyticsModule } from './analytics/analytics.module';

@Module({
  imports: [PricesModule, FeaturesModule, MonitorNetworkModule, AnalyticsModule],
})
export class AppModule {}
