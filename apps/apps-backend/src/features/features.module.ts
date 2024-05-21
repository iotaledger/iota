import { Module } from '@nestjs/common';
import { FeaturesController } from './features.controller';

@Module({
  controllers: [FeaturesController],
})
export class FeaturesModule {}
