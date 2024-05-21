import { Module } from '@nestjs/common';
import { ConfigModule } from '@nestjs/config';
import { PricesModule } from './prices/prices.module';
import { FeaturesModule } from './features/features.module';
import { MonitorNetworkModule } from './monitor-network/monitor-network.module';
import { AnalyticsModule } from './analytics/analytics.module';
@Module({
  imports: [
    PricesModule,
    FeaturesModule,
    MonitorNetworkModule,
    AnalyticsModule,
    ConfigModule.forRoot({
      isGlobal: true,
      envFilePath: '.env',
      expandVariables: true,
    }),
  ],
})
export class AppModule {}
