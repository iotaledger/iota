import { Module } from '@nestjs/common';
import { PricesController } from './prices.controller';

@Module({
  controllers: [PricesController],
})
export class PricesModule {}
